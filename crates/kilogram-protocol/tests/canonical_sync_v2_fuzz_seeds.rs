#![cfg(feature = "decoder-fuzzing")]

use std::{collections::BTreeMap, path::Path};

use kilogram_protocol::decoder_fuzzing::{
    CANONICAL_SYNC_V2_DECODER_TARGET_COUNT, CanonicalSyncV2DecoderTarget,
    exercise_canonical_sync_v2_decoder,
};
use serde_json::Value;

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
        return Err(invalid_data("invalid lowercase hexadecimal seed part"));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
        .collect()
}

fn allowed_document(name: &str) -> bool {
    matches!(
        name,
        "CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
            | "CANONICAL-SYNC-V2-VECTORS.json"
            | "CANONICAL-SYNC-SESSION-V2-VECTORS.json"
            | "CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json"
    )
}

#[test]
fn canonical_sync_v2_fuzz_seeds_are_exact_and_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(
        workspace.join("CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json"),
    )?)?;
    if manifest["milestone"] != "M1.1.73"
        || manifest["status"] != "coverage-guided-fuzz-harness-verified-campaign-not-run"
        || manifest["selector"]["target_count"].as_u64()
            != Some(u64::from(CANONICAL_SYNC_V2_DECODER_TARGET_COUNT))
        || manifest["seed_count"].as_u64() != Some(20)
        || manifest["observed"]["sustained_campaign_completed"] != false
    {
        return Err(invalid_data("fuzz campaign manifest drifted"));
    }

    let seeds = manifest["seeds"]
        .as_array()
        .ok_or_else(|| invalid_data("fuzz seed inventory is missing"))?;
    let mut documents = BTreeMap::<String, Value>::new();
    let mut seen_ids = std::collections::BTreeSet::new();

    for seed in seeds {
        let id = seed["id"]
            .as_str()
            .ok_or_else(|| invalid_data("fuzz seed ID is missing"))?;
        if !seen_ids.insert(id) {
            return Err(invalid_data(format!("duplicate fuzz seed ID {id}")));
        }
        let selector = u8::try_from(
            seed["selector"]
                .as_u64()
                .ok_or_else(|| invalid_data(format!("seed {id} selector is missing")))?,
        )?;
        let target = CanonicalSyncV2DecoderTarget::from_selector(selector)
            .ok_or_else(|| invalid_data(format!("seed {id} selector is unknown")))?;
        let mut payload = Vec::new();
        for part in seed["parts"]
            .as_array()
            .ok_or_else(|| invalid_data(format!("seed {id} parts are missing")))?
        {
            let document_name = part["document"]
                .as_str()
                .ok_or_else(|| invalid_data(format!("seed {id} document is missing")))?;
            if !allowed_document(document_name) {
                return Err(invalid_data(format!(
                    "seed {id} references forbidden document {document_name}"
                )));
            }
            if !documents.contains_key(document_name) {
                documents.insert(
                    document_name.to_owned(),
                    serde_json::from_str(&std::fs::read_to_string(workspace.join(document_name))?)?,
                );
            }
            let pointer = part["pointer"]
                .as_str()
                .ok_or_else(|| invalid_data(format!("seed {id} pointer is missing")))?;
            let value = documents[document_name]
                .pointer(pointer)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_data(format!("seed {id} pointer is invalid")))?;
            payload.extend(decode_hex(value)?);
        }
        if seed["payload_bytes"].as_u64() != Some(payload.len() as u64)
            || seed["input_bytes"].as_u64() != Some((payload.len() + 1) as u64)
        {
            return Err(invalid_data(format!("seed {id} byte count drifted")));
        }
        assert!(
            exercise_canonical_sync_v2_decoder(target, &payload),
            "golden fuzz seed {id} was rejected"
        );
    }

    assert_eq!(seeds.len(), 20);
    println!("canonical_sync_v2_fuzz_seeds={}", seeds.len());
    Ok(())
}
