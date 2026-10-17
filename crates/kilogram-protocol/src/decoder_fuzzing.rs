//! Opt-in dispatch surface for external coverage-guided decoder fuzzing.
//!
//! The default production build does not compile this module. The dedicated
//! `decoder-fuzzing` feature gives a libFuzzer target one stable place to call
//! every history-compatible canonical sync v2 strict decoder without copying
//! protocol logic into the harness.

use crate::{
    canonical_sync_history_v2::{CanonicalSyncEventCarrierV2, CanonicalSyncEventKeyV2},
    canonical_sync_responder_session_v2::decode_and_verify_signed_sync_responder_checkpoint_v2,
    canonical_sync_session_v2::decode_and_verify_signed_sync_session_checkpoint_v2,
    canonical_sync_v2::{
        decode_and_verify_signed_sync_diff_v2, decode_and_verify_signed_sync_inventory_v2,
        decode_and_verify_sync_client_frame_v2, decode_and_verify_sync_event_batch_v2,
        decode_and_verify_sync_server_frame_v2, decode_sync_complete_v2, decode_sync_pause_v2,
        decode_sync_paused_v2, decode_sync_rejected_v2,
    },
};

pub const CANONICAL_SYNC_V2_DECODER_TARGET_COUNT: u8 = 13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CanonicalSyncV2DecoderTarget {
    EventKey = 0,
    EventCarrier = 1,
    Inventory = 2,
    EventBatch = 3,
    Diff = 4,
    Complete = 5,
    Pause = 6,
    Paused = 7,
    Rejected = 8,
    ClientFrame = 9,
    ServerFrame = 10,
    SessionCheckpoint = 11,
    ResponderCheckpoint = 12,
}

impl CanonicalSyncV2DecoderTarget {
    pub const fn selector(self) -> u8 {
        self as u8
    }

    pub const fn from_selector(selector: u8) -> Option<Self> {
        match selector {
            0 => Some(Self::EventKey),
            1 => Some(Self::EventCarrier),
            2 => Some(Self::Inventory),
            3 => Some(Self::EventBatch),
            4 => Some(Self::Diff),
            5 => Some(Self::Complete),
            6 => Some(Self::Pause),
            7 => Some(Self::Paused),
            8 => Some(Self::Rejected),
            9 => Some(Self::ClientFrame),
            10 => Some(Self::ServerFrame),
            11 => Some(Self::SessionCheckpoint),
            12 => Some(Self::ResponderCheckpoint),
            _ => None,
        }
    }

    pub const fn from_fuzzer_byte(byte: u8) -> Self {
        match byte % CANONICAL_SYNC_V2_DECODER_TARGET_COUNT {
            0 => Self::EventKey,
            1 => Self::EventCarrier,
            2 => Self::Inventory,
            3 => Self::EventBatch,
            4 => Self::Diff,
            5 => Self::Complete,
            6 => Self::Pause,
            7 => Self::Paused,
            8 => Self::Rejected,
            9 => Self::ClientFrame,
            10 => Self::ServerFrame,
            11 => Self::SessionCheckpoint,
            _ => Self::ResponderCheckpoint,
        }
    }
}

/// Exercises exactly one strict decoder and reports whether it accepted the
/// input. Rejection is an ordinary fuzz outcome; a panic, abort or sanitizer
/// finding is the failure signal consumed by libFuzzer.
pub fn exercise_canonical_sync_v2_decoder(
    target: CanonicalSyncV2DecoderTarget,
    bytes: &[u8],
) -> bool {
    match target {
        CanonicalSyncV2DecoderTarget::EventKey => CanonicalSyncEventKeyV2::decode(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::EventCarrier => {
            CanonicalSyncEventCarrierV2::decode_and_verify(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::Inventory => {
            decode_and_verify_signed_sync_inventory_v2(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::EventBatch => {
            decode_and_verify_sync_event_batch_v2(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::Diff => decode_and_verify_signed_sync_diff_v2(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::Complete => decode_sync_complete_v2(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::Pause => decode_sync_pause_v2(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::Paused => decode_sync_paused_v2(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::Rejected => decode_sync_rejected_v2(bytes).is_ok(),
        CanonicalSyncV2DecoderTarget::ClientFrame => {
            decode_and_verify_sync_client_frame_v2(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::ServerFrame => {
            decode_and_verify_sync_server_frame_v2(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::SessionCheckpoint => {
            decode_and_verify_signed_sync_session_checkpoint_v2(bytes).is_ok()
        }
        CanonicalSyncV2DecoderTarget::ResponderCheckpoint => {
            decode_and_verify_signed_sync_responder_checkpoint_v2(bytes).is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_are_closed_and_fuzzer_bytes_cover_every_target() -> Result<(), &'static str> {
        for selector in 0..CANONICAL_SYNC_V2_DECODER_TARGET_COUNT {
            let target = CanonicalSyncV2DecoderTarget::from_selector(selector)
                .ok_or("known selector was rejected")?;
            assert_eq!(target.selector(), selector);
            assert_eq!(
                CanonicalSyncV2DecoderTarget::from_fuzzer_byte(selector),
                target
            );
            assert!(!exercise_canonical_sync_v2_decoder(target, &[]));
        }
        assert!(
            CanonicalSyncV2DecoderTarget::from_selector(CANONICAL_SYNC_V2_DECODER_TARGET_COUNT)
                .is_none()
        );
        assert_eq!(
            CanonicalSyncV2DecoderTarget::from_fuzzer_byte(255),
            CanonicalSyncV2DecoderTarget::from_selector(
                255 % CANONICAL_SYNC_V2_DECODER_TARGET_COUNT
            )
            .ok_or("modulo selector was rejected")?
        );
        Ok(())
    }
}
