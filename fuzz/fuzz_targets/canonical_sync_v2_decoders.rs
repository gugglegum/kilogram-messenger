#![no_main]

use kilogram_protocol::decoder_fuzzing::{
    CanonicalSyncV2DecoderTarget, exercise_canonical_sync_v2_decoder,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let Some((&selector, payload)) = input.split_first() else {
        return;
    };
    let target = CanonicalSyncV2DecoderTarget::from_fuzzer_byte(selector);
    std::hint::black_box(exercise_canonical_sync_v2_decoder(target, payload));
});
