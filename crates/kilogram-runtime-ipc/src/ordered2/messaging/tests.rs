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
