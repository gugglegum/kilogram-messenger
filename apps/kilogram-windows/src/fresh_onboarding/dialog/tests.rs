use super::*;
use anyhow::{Context, Result};

fn action_reply(
    action: Action,
    expected_slot: u8,
    expected_text: &str,
) -> Option<mpsc::SyncSender<Submission>> {
    let Action::Send { slot, text, reply } = action else {
        return None;
    };
    assert_eq!(slot, expected_slot);
    assert!(text.as_str() == expected_text);
    Some(reply)
}

#[test]
fn cs33b_draft_survives_rejection_and_only_its_revision_is_cleared() -> Result<()> {
    let mut dialog = Dialog::default();
    dialog.drafts[0].text.push_str("  first\n");
    dialog.drafts[1].text.push_str("second");
    assert!(!dialog.request_send(|_| false));
    assert!(dialog.submission.is_none());
    assert!(dialog.drafts[0].text.as_str() == "  first\n");

    let mut reply = None;
    assert!(dialog.request_send(|action| {
        reply = action_reply(action, 0, "  first\n");
        true
    }));
    assert!(!dialog.request_send(|_| true));
    reply
        .take()
        .context("First send action")?
        .send(Submission::Retained)?;
    assert!(dialog.poll_submission().is_none());
    assert!(dialog.drafts[0].text.as_str() == "  first\n");

    assert!(dialog.request_send(|action| {
        reply = action_reply(action, 0, "  first\n");
        true
    }));
    dialog.drafts[0].text.push_str("changed");
    dialog.drafts[0].revision += 1;
    dialog.select(1);
    assert!(!dialog.request_send(|_| true));
    reply
        .take()
        .context("Second send action")?
        .send(Submission::Protected)?;
    assert!(dialog.poll_submission().is_none());
    assert!(dialog.drafts[0].text.as_str() == "  first\nchanged");
    assert!(dialog.drafts[1].text.as_str() == "second");

    assert!(dialog.request_send(|action| {
        reply = action_reply(action, 1, "second");
        true
    }));
    dialog.select(0);
    dialog.reset_runtime();
    reply
        .take()
        .context("Other slot send action")?
        .send(Submission::Protected)?;
    assert_eq!(dialog.poll_submission(), Some(1));
    assert!(dialog.drafts[1].text.is_empty());
    assert!(dialog.drafts[0].text.as_str() == "  first\nchanged");
    Ok(())
}

#[test]
fn cs33b_draft_limit_is_whole_utf8_bytes() {
    let mut text = "x".repeat(TEXT_CAP - 3);
    let mut edit = BoundedEdit {
        text: &mut text,
        rejected: false,
    };
    assert_eq!(edit.insert_text("é", 0), 1);
    assert_eq!(edit.insert_text("é", 0), 0);
    assert!(edit.rejected);
    assert!(edit.as_str().starts_with("éx"));
    let previous = edit.as_str().to_owned();
    edit.replace_with(&"é".repeat(TEXT_CAP / 2 + 1));
    assert!(edit.as_str() == previous);
    assert_eq!(edit.insert_text("x", 0), 1);
    assert_eq!(edit.as_str().len(), TEXT_CAP);
}

fn row(index: u8, role: u8) -> HistoryRow2 {
    HistoryRow2 {
        event: [index; 32],
        role,
        ordinal: index.into(),
        sequence: index.into(),
        request: [7; 16],
        text: Zeroizing::new(" exact message\n".into()),
    }
}

#[test]
fn cs33b_history_preserves_causal_order_and_checks_exact_preview() {
    let mut history = History::default();
    assert!(history.merge((1..=24).map(|i| row(i, 0)).collect(), true));
    assert!(history.merge((24..=42).map(|i| row(i, 0)).collect(), false));
    let ordinals: Vec<_> = history.rows.iter().map(|r| r.data.ordinal).collect();
    assert_eq!(ordinals, (11..=42).collect::<Vec<_>>());
    assert!(!history.merge(vec![row(42, 1)], false));
    assert_eq!(history.rows.len(), MAX_HISTORY_ROWS);
    history.fail(MessageError2::HistoryCursorInvalid);
    assert!(history.refresh_required);
    assert_eq!(history.rows.back().map(|row| row.data.ordinal), Some(42));

    let key = MessageKey {
        slot: 0,
        request: [7; 16],
        event: [42; 32],
    };
    assert!(matches_preview(&row(42, 0), 0, key, " exact message\n"));
    assert!(!matches_preview(&row(42, 1), 0, key, " exact message\n"));
    assert!(!matches_preview(&row(41, 0), 0, key, " exact message\n"));
    assert!(!matches_preview(&row(42, 0), 0, key, "exact message"));
}

#[test]
fn cs33b_stale_pages_and_delivery_cannot_change_the_current_view() {
    let mut dialog = Dialog::default();
    let scope = DialogScope {
        instance: [1; 32],
        contexts: [[2; 32], [3; 32]],
        own_role: 0,
    };
    dialog.bind(scope);
    let (reply, receiver) = mpsc::sync_channel(1);
    dialog.page = Some(PageFlight {
        scope,
        slot: 0,
        replace: false,
        reply: receiver,
    });
    assert!(reply.send(Err(MessageError2::HistoryCursorInvalid)).is_ok());
    dialog.select(1);
    dialog.poll_page();
    assert!(dialog.history.error.is_none());
    dialog.history.merge(vec![row(1, 0), row(2, 1)], true);
    let key = MessageKey {
        slot: 1,
        request: [7; 16],
        event: [1; 32],
    };
    let mut respond = |flight_scope, flight_key, status| {
        let (reply, receiver) = mpsc::sync_channel(1);
        dialog.delivery = Some(DeliveryFlight {
            scope: flight_scope,
            key: flight_key,
            reply: receiver,
        });
        assert!(reply.send(status).is_ok());
        dialog.poll_delivery(Some(key));
        (
            dialog.history.rows[0].delivery,
            dialog.history.rows[1].delivery,
        )
    };
    let stale = DialogScope {
        instance: [4; 32],
        ..scope
    };
    let peer = MessageKey {
        event: [2; 32],
        ..key
    };
    assert!(respond(stale, key, Some(Delivery2::Delivered)) == (None, None));
    assert!(respond(scope, peer, Some(Delivery2::Delivered)) == (None, None));
    assert!(respond(scope, key, Some(Delivery2::Delivered)) == (Some(Delivery2::Delivered), None));
    assert!(respond(scope, key, None) == (None, None));
    dialog.reset_runtime();
    dialog.bind(stale);
    assert!(dialog.history.rows.is_empty() && dialog.preview_delivery.is_none());
    assert_eq!(delivery_label(None, false), "Delivery not confirmed");
    assert_eq!(
        delivery_label(Some(Delivery2::Delivered), false),
        "Delivered earlier"
    );
}
