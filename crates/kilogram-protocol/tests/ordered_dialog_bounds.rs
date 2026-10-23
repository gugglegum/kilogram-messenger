//! Encoding/credit vectors only. Opaque maximum-size payload placeholders are
//! intentionally not credential/session fixtures or process acceptance evidence.
use kilogram_identity::DeviceIdentity;
use kilogram_protocol::ordered_dialog_v1::{self as wire, Frame, a, b, u};

#[test]
fn maximal_ordered_wire_payloads_fit_normal_batch_and_reserved_head() -> wire::Result<()> {
    let signer = DeviceIdentity::from_secret_bytes([88; 32]);
    let catalog =
        |count: usize| a((0..count).map(|i| a([b([i as u8; 32]), b(vec![0; wire::MAX_AUTH])])));
    let events = |count: usize| a((0..count).map(|_| b(vec![0; wire::MAX_EVENT])));
    let deliver = |count| {
        Frame::signed(
            10,
            [1; 32],
            [2; 16],
            a([
                b([3; 32]),
                b([4; 32]),
                b([5; 32]),
                u(0),
                catalog(count),
                events(count),
            ]),
            &signer,
        )
    };
    let normal = deliver(8)?.raw()?.len() + 4;
    let head = Frame::signed(
        12,
        [1; 32],
        [2; 16],
        a([
            b([3; 32]),
            b([4; 32]),
            b([5; 32]),
            u(0),
            u(2),
            u(3),
            a([u(512), a([b([6; 32])])]),
            catalog(1),
            events(1),
            u(1),
        ]),
        &signer,
    )?
    .raw()?
    .len()
        + 4;
    let stored = wire::MAX_EVENT + 2 * wire::MAX_AUTH + 1024;
    assert!(normal <= wire::MAX_WIRE);
    assert!(head <= 68 * 1024 && stored <= 64 * 1024);
    assert!(deliver(32).is_err());
    println!(
        "OD1 maximal full_wire_normal8_bytes={normal} head_wire_bytes={head} head_stored_bytes={stored} full32_refused=true"
    );
    Ok(())
}
