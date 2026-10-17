use ed25519_dalek::{Signer, SigningKey};
use kilogram_protocol::{
    canonical_sync_history_v2::{CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2},
    canonical_sync_responder_session_v2::{
        CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderCheckpointContentV2,
        CanonicalSyncResponderNextActionV2, decode_and_verify_signed_sync_responder_checkpoint_v2,
        encode_signed_sync_responder_checkpoint_v2, encode_sync_responder_checkpoint_content_v2,
        sync_responder_checkpoint_id_v2, sync_responder_checkpoint_signing_bytes_v2,
    },
    canonical_sync_session_v2::decode_and_verify_signed_sync_session_checkpoint_v2,
    canonical_sync_v2::{
        decode_and_verify_signed_sync_diff_v2, decode_and_verify_signed_sync_inventory_v2,
        decode_and_verify_sync_client_frame_v2, decode_and_verify_sync_event_batch_v2,
        decode_and_verify_sync_server_frame_v2, decode_sync_complete_v2, decode_sync_pause_v2,
        decode_sync_paused_v2, decode_sync_rejected_v2,
    },
};
use serde_json::Value;

const MINIMUM_MUTATIONS: usize = 2_500;
const EXPECTED_MUTATIONS: usize = 2_863;
const MAXIMUM_MUTATIONS: usize = 4_000;
const MAXIMUM_FIXTURE_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_GRID_POSITIONS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationPolicy {
    StructuralOnly,
    AuthenticatedBitFlips,
}

impl MutationPolicy {
    const fn manifest_name(self) -> &'static str {
        match self {
            Self::StructuralOnly => "structural-only",
            Self::AuthenticatedBitFlips => "authenticated-bit-flips",
        }
    }
}

#[derive(Clone, Copy)]
enum Decoder {
    EventKey,
    EventCarrier,
    Inventory,
    EventBatch,
    Diff,
    Complete,
    Pause,
    Paused,
    Rejected,
    ClientFrame,
    ServerFrame,
    SessionCheckpoint,
    ResponderCheckpoint,
}

struct Fixture {
    id: &'static str,
    decoder: Decoder,
    policy: MutationPolicy,
    bytes: Vec<u8>,
}

fn accepts(decoder: Decoder, bytes: &[u8]) -> bool {
    match decoder {
        Decoder::EventKey => CanonicalSyncEventKeyV2::decode(bytes).is_ok(),
        Decoder::EventCarrier => CanonicalSyncEventCarrierV2::decode_and_verify(bytes).is_ok(),
        Decoder::Inventory => decode_and_verify_signed_sync_inventory_v2(bytes).is_ok(),
        Decoder::EventBatch => decode_and_verify_sync_event_batch_v2(bytes).is_ok(),
        Decoder::Diff => decode_and_verify_signed_sync_diff_v2(bytes).is_ok(),
        Decoder::Complete => decode_sync_complete_v2(bytes).is_ok(),
        Decoder::Pause => decode_sync_pause_v2(bytes).is_ok(),
        Decoder::Paused => decode_sync_paused_v2(bytes).is_ok(),
        Decoder::Rejected => decode_sync_rejected_v2(bytes).is_ok(),
        Decoder::ClientFrame => decode_and_verify_sync_client_frame_v2(bytes).is_ok(),
        Decoder::ServerFrame => decode_and_verify_sync_server_frame_v2(bytes).is_ok(),
        Decoder::SessionCheckpoint => {
            decode_and_verify_signed_sync_session_checkpoint_v2(bytes).is_ok()
        }
        Decoder::ResponderCheckpoint => {
            decode_and_verify_signed_sync_responder_checkpoint_v2(bytes).is_ok()
        }
    }
}

fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    ))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(invalid_data("invalid lowercase hexadecimal"));
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
            .ok_or_else(|| invalid_data(format!("missing vector pointer {pointer}")))?,
    )
}

fn pointer_fixed<const N: usize>(
    document: &Value,
    pointer: &str,
) -> Result<[u8; N], Box<dyn std::error::Error>> {
    pointer_hex(document, pointer)?
        .try_into()
        .map_err(|bytes: Vec<u8>| invalid_data(format!("{pointer} has {} bytes", bytes.len())))
}

fn pointer_usize(document: &Value, pointer: &str) -> Result<usize, Box<dyn std::error::Error>> {
    usize::try_from(
        document
            .pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_data(format!("missing numeric vector pointer {pointer}")))?,
    )
    .map_err(|error| error.into())
}

fn bounded_positions(length: usize) -> Vec<usize> {
    if length <= MAXIMUM_GRID_POSITIONS {
        return (0..length).collect();
    }
    (0..MAXIMUM_GRID_POSITIONS)
        .map(|index| index * length / MAXIMUM_GRID_POSITIONS)
        .collect()
}

fn reconstructed_event_batch(
    sync: &Value,
    history: &Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = pointer_hex(sync, "/expected/event_batch/prefix_hex")?;
    bytes.extend(pointer_hex(history, "/expected/legacy_carrier_cbor_hex")?);
    if bytes.len() != pointer_usize(sync, "/expected/event_batch/encoded_length")? {
        return Err(invalid_data("event-batch vector length changed"));
    }
    Ok(bytes)
}

fn reconstructed_event_frame(
    sync: &Value,
    history: &Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = pointer_hex(sync, "/expected/client_frames/events/prefix_hex")?;
    bytes.extend(reconstructed_event_batch(sync, history)?);
    if bytes.len() != pointer_usize(sync, "/expected/client_frames/events/encoded_length")? {
        return Err(invalid_data("client event-frame vector length changed"));
    }
    Ok(bytes)
}

fn responder_checkpoint_vector(vectors: &Value) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let signing_key = SigningKey::from_bytes(&pointer_fixed(
        vectors,
        "/input/responder_device_secret_key_hex",
    )?);
    let content = CanonicalSyncResponderCheckpointContentV2 {
        protocol_identifier: pointer_fixed(vectors, "/expected/protocol_identifier_hex")?,
        conversation_id: pointer_fixed(vectors, "/input/conversation_id_hex")?,
        responder_device_id: signing_key.verifying_key().to_bytes(),
        requester_device_id: pointer_fixed(vectors, "/input/requester_device_id_hex")?,
        session_binding: pointer_fixed(vectors, "/input/session_binding_hex")?,
        checkpoint_sequence: 1,
        sync_round: 1,
        predecessor_checkpoint_id: None,
        received_event_keys: Vec::new(),
        sent_event_keys: Vec::new(),
        continuation_required: false,
        next_action: CanonicalSyncResponderNextActionV2::ReceiveInventory,
        paused: false,
    };
    if content.responder_device_id != pointer_fixed(vectors, "/expected/responder_device_id_hex")? {
        return Err(invalid_data("responder Device ID vector changed"));
    }
    let content_bytes = encode_sync_responder_checkpoint_content_v2(&content)?;
    if content_bytes != pointer_hex(vectors, "/expected/content_cbor_hex")? {
        return Err(invalid_data("responder checkpoint content vector changed"));
    }
    let signing_preimage = sync_responder_checkpoint_signing_bytes_v2(&content)?;
    if signing_preimage != pointer_hex(vectors, "/expected/signing_preimage_hex")? {
        return Err(invalid_data(
            "responder checkpoint signing preimage changed",
        ));
    }
    let signature = signing_key.sign(&signing_preimage).to_bytes();
    if signature != pointer_fixed(vectors, "/expected/signature_hex")? {
        return Err(invalid_data(
            "responder checkpoint signature vector changed",
        ));
    }
    let checkpoint = CanonicalSignedSyncResponderCheckpointV2 { content, signature };
    let signed = encode_signed_sync_responder_checkpoint_v2(&checkpoint)?;
    if signed != pointer_hex(vectors, "/expected/signed_cbor_hex")? {
        return Err(invalid_data("signed responder checkpoint vector changed"));
    }
    if sync_responder_checkpoint_id_v2(&checkpoint)?
        != pointer_fixed(vectors, "/expected/checkpoint_id_hex")?
    {
        return Err(invalid_data("responder checkpoint ID vector changed"));
    }
    Ok(signed)
}

#[test]
fn bounded_canonical_sync_v2_decoder_mutation_corpus_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let manifest: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-V2-DECODER-MUTATIONS.json"
    )))?;
    let history: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
    )))?;
    let sync: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-V2-VECTORS.json"
    )))?;
    let session: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-SESSION-V2-VECTORS.json"
    )))?;
    let responder: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json"
    )))?;

    let fixtures = vec![
        Fixture {
            id: "legacy-event-key-v2",
            decoder: Decoder::EventKey,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&history, "/expected/legacy_key_cbor_hex")?,
        },
        Fixture {
            id: "canonical-event-key-v2",
            decoder: Decoder::EventKey,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&history, "/expected/canonical_key_cbor_hex")?,
        },
        Fixture {
            id: "legacy-event-carrier-v2",
            decoder: Decoder::EventCarrier,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&history, "/expected/legacy_carrier_cbor_hex")?,
        },
        Fixture {
            id: "canonical-event-carrier-v2",
            decoder: Decoder::EventCarrier,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&history, "/expected/canonical_carrier_cbor_hex")?,
        },
        Fixture {
            id: "signed-sync-inventory-v2",
            decoder: Decoder::Inventory,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&sync, "/expected/inventory/signed_cbor_hex")?,
        },
        Fixture {
            id: "sync-event-batch-v2",
            decoder: Decoder::EventBatch,
            policy: MutationPolicy::StructuralOnly,
            bytes: reconstructed_event_batch(&sync, &history)?,
        },
        Fixture {
            id: "signed-sync-diff-v2",
            decoder: Decoder::Diff,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&sync, "/expected/diff/signed_cbor_hex")?,
        },
        Fixture {
            id: "sync-complete-v2",
            decoder: Decoder::Complete,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/complete/cbor_hex")?,
        },
        Fixture {
            id: "sync-pause-v2",
            decoder: Decoder::Pause,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/pause/cbor_hex")?,
        },
        Fixture {
            id: "sync-paused-v2",
            decoder: Decoder::Paused,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/paused/cbor_hex")?,
        },
        Fixture {
            id: "sync-rejected-v2",
            decoder: Decoder::Rejected,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/rejected/cbor_hex")?,
        },
        Fixture {
            id: "sync-client-inventory-frame-v2",
            decoder: Decoder::ClientFrame,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&sync, "/expected/client_frames/inventory_cbor_hex")?,
        },
        Fixture {
            id: "sync-client-events-frame-v2",
            decoder: Decoder::ClientFrame,
            policy: MutationPolicy::StructuralOnly,
            bytes: reconstructed_event_frame(&sync, &history)?,
        },
        Fixture {
            id: "sync-client-pause-frame-v2",
            decoder: Decoder::ClientFrame,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/client_frames/pause_cbor_hex")?,
        },
        Fixture {
            id: "sync-server-diff-frame-v2",
            decoder: Decoder::ServerFrame,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&sync, "/expected/server_frames/diff_cbor_hex")?,
        },
        Fixture {
            id: "sync-server-complete-frame-v2",
            decoder: Decoder::ServerFrame,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/server_frames/complete_cbor_hex")?,
        },
        Fixture {
            id: "sync-server-rejected-frame-v2",
            decoder: Decoder::ServerFrame,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/server_frames/rejected_cbor_hex")?,
        },
        Fixture {
            id: "sync-server-paused-frame-v2",
            decoder: Decoder::ServerFrame,
            policy: MutationPolicy::StructuralOnly,
            bytes: pointer_hex(&sync, "/expected/server_frames/paused_cbor_hex")?,
        },
        Fixture {
            id: "signed-sync-session-checkpoint-v2",
            decoder: Decoder::SessionCheckpoint,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: pointer_hex(&session, "/expected/checkpoints/3/signed_cbor_hex")?,
        },
        Fixture {
            id: "signed-sync-responder-checkpoint-v2",
            decoder: Decoder::ResponderCheckpoint,
            policy: MutationPolicy::AuthenticatedBitFlips,
            bytes: responder_checkpoint_vector(&responder)?,
        },
    ];

    if manifest["milestone"] != "M1.1.72"
        || manifest["status"] != "bounded-canonical-sync-v2-regression-corpus"
        || manifest["coverage_guided"] != false
        || manifest["compiler_instrumentation"] != false
        || manifest["fixture_count"].as_u64() != Some(fixtures.len() as u64)
        || manifest["minimum_mutations"].as_u64() != Some(MINIMUM_MUTATIONS as u64)
        || manifest["expected_mutations"].as_u64() != Some(EXPECTED_MUTATIONS as u64)
        || manifest["maximum_mutations"].as_u64() != Some(MAXIMUM_MUTATIONS as u64)
    {
        return Err(invalid_data("sync-v2 mutation manifest drifted"));
    }
    let manifest_fixtures = manifest["fixtures"]
        .as_array()
        .ok_or_else(|| invalid_data("sync-v2 mutation fixture inventory is missing"))?;
    for (fixture, expected) in fixtures.iter().zip(manifest_fixtures) {
        if expected["id"].as_str() != Some(fixture.id)
            || expected["mutation_policy"].as_str() != Some(fixture.policy.manifest_name())
        {
            return Err(invalid_data(format!(
                "sync-v2 mutation fixture inventory drifted at {}",
                fixture.id
            )));
        }
        assert!(
            accepts(fixture.decoder, &fixture.bytes),
            "golden {} fixture was rejected",
            fixture.id
        );
        assert!(
            fixture.bytes.len() <= MAXIMUM_FIXTURE_BYTES,
            "golden {} fixture is unbounded",
            fixture.id
        );
    }

    let mut mutations = 0usize;
    for fixture in &fixtures {
        for length in bounded_positions(fixture.bytes.len()) {
            assert!(
                !accepts(fixture.decoder, &fixture.bytes[..length]),
                "{} accepted truncation at {length}",
                fixture.id
            );
            mutations += 1;
        }
        if fixture.policy == MutationPolicy::AuthenticatedBitFlips {
            for index in bounded_positions(fixture.bytes.len()) {
                let mut changed = fixture.bytes.clone();
                changed[index] ^= 1;
                assert!(
                    !accepts(fixture.decoder, &changed),
                    "{} accepted authenticated bit flip at {index}",
                    fixture.id
                );
                mutations += 1;
            }
        }
        let mut trailing = fixture.bytes.clone();
        trailing.push(0);
        assert!(
            !accepts(fixture.decoder, &trailing),
            "{} accepted trailing data",
            fixture.id
        );
        mutations += 1;

        let mut non_minimal = Vec::with_capacity(fixture.bytes.len() + 1);
        non_minimal.push(fixture.bytes[0]);
        non_minimal.extend_from_slice(&[0x18, 0x01]);
        non_minimal.extend_from_slice(&fixture.bytes[2..]);
        assert!(
            !accepts(fixture.decoder, &non_minimal),
            "{} accepted non-minimal version",
            fixture.id
        );
        mutations += 1;

        let mut indefinite = fixture.bytes.clone();
        indefinite[0] = 0x9f;
        indefinite.push(0xff);
        assert!(
            !accepts(fixture.decoder, &indefinite),
            "{} accepted an indefinite array",
            fixture.id
        );
        mutations += 1;

        let mut reserved = fixture.bytes.clone();
        reserved[0] = 0x9c;
        assert!(
            !accepts(fixture.decoder, &reserved),
            "{} accepted a reserved CBOR head",
            fixture.id
        );
        mutations += 1;
    }

    let oversized = [
        0x83, 0x01, 0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    for decoder in [
        Decoder::EventKey,
        Decoder::EventCarrier,
        Decoder::Inventory,
        Decoder::EventBatch,
        Decoder::Diff,
        Decoder::Complete,
        Decoder::Pause,
        Decoder::Paused,
        Decoder::Rejected,
        Decoder::ClientFrame,
        Decoder::ServerFrame,
        Decoder::SessionCheckpoint,
        Decoder::ResponderCheckpoint,
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
    eprintln!("canonical_sync_v2_decoder_mutations={mutations}");
    Ok(())
}
