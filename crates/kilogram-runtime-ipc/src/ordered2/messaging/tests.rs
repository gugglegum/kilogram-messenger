use super::*;
use anyhow::{Context, ensure};
use serde_json::json;
use wire::a;

#[test]
fn cs33a_message_replies_are_typed_and_sanitized() -> anyhow::Result<()> {
    let response = |version, op, value| {
        wire::encode(&a([u(version), u(op), value]), 1024)
            .map(|response| RuntimeIpcResponse::OrderedDialog { response })
    };
    ensure!(request_status(&a([]))?.is_none(), "Absent shape");
    for (value, event, expected) in [
        (0, None, Delivery2::Queued),
        (1, Some([1; 32]), Delivery2::Materialized),
        (2, Some([2; 32]), Delivery2::Delivered),
    ] {
        let body = opt(Some(a([u(9), opt(event.map(b)), u(value)])));
        let parsed = request_status(&message_reply(3, response(2, 3, body)?)?)?
            .context("Missing request status")?;
        ensure!(
            parsed.delivery == expected
                && parsed.receipt.event == event
                && parsed.receipt.queue_sequence == 9,
            "Wrong request status"
        );
    }
    for (value, event) in [(0, Some([1; 32])), (1, None), (2, None), (3, Some([1; 32]))] {
        ensure!(
            request_status(&opt(Some(a([u(9), opt(event.map(b)), u(value)])))).is_err(),
            "Invalid status shape accepted"
        );
    }
    for envelope in [
        response(1, 3, a([]))?,
        response(2, 4, a([]))?,
        RuntimeIpcResponse::ShutdownAccepted,
    ] {
        ensure!(message_reply(3, envelope).err() == Some(MessageError2::InvalidReply));
    }
    let error = |message: &str| RuntimeIpcResponse::Error {
        message: message.to_owned(),
    };
    ensure!(
        message_reply(2, error("RequestIdConflict")).err()
            == Some(MessageError2::RequestIdConflict)
    );
    ensure!(
        message_reply(4, error("InvalidCursor")).err() == Some(MessageError2::HistoryCursorInvalid)
    );
    ensure!(
        message_reply(4, error("SnapshotUnavailable")).err()
            == Some(MessageError2::HistoryCursorInvalid)
    );
    ensure!(
        message_reply(3, error("HistoryUnavailable")).err()
            == Some(MessageError2::HistoryUnavailable)
    );
    ensure!(
        message_reply(2, error("RequestIdConflict: example")).err()
            == Some(MessageError2::Unavailable)
    );
    Ok(())
}

fn cursor(byte: u8) -> String {
    let mut raw = [byte; CURSOR_BYTES];
    raw[..CURSOR_HEADER.len()].copy_from_slice(CURSOR_HEADER);
    URL_SAFE_NO_PAD.encode(raw)
}

#[test]
fn cs33a_history_cursor_count_shape_and_caps() -> anyhow::Result<()> {
    let first = json!({"id":"01".repeat(32),"role":0,"ordinal":1,"sequence":1,"request":"34".repeat(16),"text":" message "});
    let second = json!({"id":"02".repeat(32),"role":1,"ordinal":2,"sequence":2,"request":"34".repeat(16),"text":"\u{1}a"});
    let mut v = json!({"version":2,"view":"43".repeat(32),"F":[2,3],"finished":false,"cursor":cursor(0),"items":[first,second]});
    let context = [9; 32];
    let decode = |v: &serde_json::Value, before, limit| -> anyhow::Result<HistoryPage2> {
        Ok(history_page(
            &serde_json::to_vec(v)?,
            context,
            before,
            limit,
        )?)
    };
    let page = decode(&v, None, 32)?;
    ensure!(
        page.items.len() == 2 && !page.finished,
        "Short page is not EOF"
    );
    ensure!(
        page.items[0].text.as_str() == " message ",
        "Text normalization"
    );
    ensure!(decode(&v, None, 1).is_err(), "Requested count not enforced");
    ensure!(
        decode(&v, Some(&page.cursor), 32).is_err(),
        "Unchanged cursor advanced"
    );
    v["cursor"] = cursor(1).into();
    decode(&v, Some(&page.cursor), 32)?;
    for (field, bad) in [
        ("F", json!([0, 3])),
        ("F", json!([3, 3])),
        ("view", json!("AA".repeat(32))),
        ("view", json!("a0".repeat(32))),
        ("cursor", json!(format!("{}=", cursor(1)))),
        ("items", json!([])),
        ("version", json!(1)),
        ("unexpected", json!(true)),
    ] {
        let mut invalid = v.clone();
        invalid[field] = bad;
        ensure!(
            decode(&invalid, Some(&page.cursor), 32).is_err(),
            "Invalid page accepted"
        );
    }
    for (field, bad) in [
        ("ordinal", json!(0)),
        ("ordinal", json!(2)),
        ("role", json!(2)),
        ("request", json!("gg".repeat(16))),
        ("text", json!("x".repeat(4097))),
    ] {
        let mut invalid = v.clone();
        invalid["items"][0][field] = bad;
        ensure!(
            decode(&invalid, None, 32).is_err(),
            "Invalid history row accepted"
        );
    }
    let mut invalid = v.clone();
    invalid["items"][1]["id"] = invalid["items"][0]["id"].clone();
    ensure!(
        decode(&invalid, None, 32).is_err(),
        "Duplicate event accepted"
    );
    ensure!(history_page(&vec![b' '; HISTORY_JSON_CAP + 1], context, None, 32).is_err());
    Ok(())
}

fn position_value(context: Id, view: Id, frontiers: [u64; 2], next: [u64; 2]) -> Value {
    // Fixtures exercise public metadata checking, opaque to users of Client2.
    let mut raw = [0u8; CURSOR_BYTES];
    raw[..8].copy_from_slice(CURSOR_HEADER);
    raw[40..72].copy_from_slice(&view);
    raw[72..104].copy_from_slice(&context);
    for (bytes, ordinal) in raw[211..243]
        .chunks_exact_mut(8)
        .zip(frontiers.into_iter().chain(next))
    {
        bytes.copy_from_slice(&ordinal.to_be_bytes());
    }
    a([
        b(context),
        b(view),
        a(frontiers.map(u)),
        a(next.map(u)),
        b(raw),
    ])
}

#[test]
fn usable_peer_opaque_checkpoint_preserves_completed_and_partial_positions() -> anyhow::Result<()> {
    let context=[1;32];let view=[2;32];let cut=[4,7];
    let completed=history_position(&position_value(context,view,cut,cut),context,None)?.cursor;
    let resumed=HistoryCursor2::from_checkpoint(&completed.checkpoint())?;
    ensure!(resumed==completed && resumed.completed());
    let delta=history_position(&position_value(context,view,[6,8],cut),context,Some(&completed))?.cursor;
    let resumed=HistoryCursor2::from_checkpoint(&delta.checkpoint())?;
    ensure!(resumed==delta && !resumed.completed());
    for bad in ["".to_owned(),completed.checkpoint()+" ","a".repeat(368)] {
        ensure!(HistoryCursor2::from_checkpoint(&bad).is_err());
    }
    let mut raw=completed.raw;raw[0]^=1;
    ensure!(HistoryCursor2::from_checkpoint(&URL_SAFE_NO_PAD.encode(raw)).is_err());
    let mut raw=completed.raw;raw[227..235].copy_from_slice(&5u64.to_be_bytes());
    ensure!(HistoryCursor2::from_checkpoint(&URL_SAFE_NO_PAD.encode(raw)).is_err());
    Ok(())
}

#[test]
fn t11e1_position_replies_match_cursor_and_previous_cut() -> anyhow::Result<()> {
    let context = [1; 32];
    let view = [2; 32];
    let old = [4, 7];
    let new = [6, 8];
    let anchor = history_position(&position_value(context, view, old, old), context, None)?;
    let follow = position_value(context, view, new, old);
    let decode = |v: &Value| history_position(v, context, Some(&anchor.cursor));
    let next = decode(&follow)?;
    ensure!(
        next.context == context && next.view == view && next.frontiers == new && next.next == old
    );
    for value in [
        position_value([3; 32], view, new, old),
        position_value(context, [3; 32], new, old),
        position_value(context, view, [6, 6], old),
        position_value(context, view, [3, 8], old),
        position_value(context, view, new, [4, 6]),
        position_value(context, view, new, [0, 7]),
        a([]),
    ] {
        ensure!(decode(&value).err() == Some(MessageError2::InvalidReply));
    }
    ensure!(history_position(&follow, context, None).is_err());
    let fields = follow.fields(5)?;
    for offset in [0, 40, 72, 211, 227, 234] {
        let mut values = fields.to_vec();
        let mut bytes = fields[4].fixed::<CURSOR_BYTES>()?;
        bytes[offset] ^= 1;
        values[4] = b(bytes);
        ensure!(decode(&a(values)).err() == Some(MessageError2::InvalidReply));
    }
    let mut values = fields.to_vec();
    values[4] = b([0; CURSOR_BYTES - 1]);
    ensure!(decode(&a(values)).err() == Some(MessageError2::InvalidReply));
    Ok(())
}

#[test]
fn t11e1_position_wire_max_includes_framing_and_legacy_rejects() -> anyhow::Result<()> {
    let context = [1; 32];
    let max_ordinals = [u64::MAX; 2];
    let position = position_value(context, [2; 32], max_ordinals, max_ordinals);
    let anchor = history_position(&position, context, None)?;
    for opcode in [HISTORY_ANCHOR_OPCODE, HISTORY_FOLLOW_OPCODE] {
        let encode = |version, op| {
            wire::encode(
                &a([u(version), u(op), position.clone()]),
                HISTORY_POSITION_REPLY_CAP,
            )
        };
        let bytes = encode(2, opcode)?;
        ensure!(bytes.len() == HISTORY_POSITION_REPLY_CAP);
        let max_response = RuntimeIpcResponse::OrderedDialog { response: bytes };
        let frame_bytes = 4 + postcard::to_allocvec(&max_response)?.len();
        ensure!(frame_bytes == 395 && frame_bytes <= 512);
        let fields = message_reply(opcode, max_response)?;
        let previous = (opcode == HISTORY_FOLLOW_OPCODE).then_some(&anchor.cursor);
        ensure!(history_position(&fields, context, previous)?.cursor == anchor.cursor);
        for response in [
            encode(1, opcode)?,
            encode(2, 4)?,
            vec![0; HISTORY_POSITION_REPLY_CAP + 1],
        ] {
            ensure!(
                message_reply(opcode, RuntimeIpcResponse::OrderedDialog { response }).err()
                    == Some(MessageError2::InvalidReply)
            );
        }
        for (reason, expected) in [
            (
                "UnsupportedOperation",
                MessageError2::HistoryPositionUnsupported,
            ),
            ("HistoryUnavailable", MessageError2::HistoryUnavailable),
        ] {
            let reply = RuntimeIpcResponse::Error {
                message: reason.to_owned(),
            };
            ensure!(message_reply(opcode, reply).err() == Some(expected));
        }
    }
    Ok(())
}
