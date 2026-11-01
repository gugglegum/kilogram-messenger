use super::*;
fn request() -> Request {
    Request {
        incarnation: state::hex(&[2; 32]),
        lease_generation: 7,
        from: 16,
    }
}
fn gate() -> Gate {
    Gate {
        nonce: [2; 32],
        binding: [3; 32],
        context: [4; 32],
        generation: 7,
        role: 1,
        active: true,
        received: 16,
        normal_first: 26,
        debt: Some((16, [16; 32])),
        held: false,
    }
}
fn ticket() -> Result<Ticket> {
    gate().ticket(&request())
}
fn plan() -> Json {
    json!({"context":state::hex(&[4;32]),"lane":1,"phase":"mixed","max_pause_ms":1000,"retry_live":{"incarnation":state::hex(&[2;32]),"lease_generation":7,"from":16}})
}
#[test]
fn s54_r2_retry_schema_is_debug_mixed_b_and_old_source_only() -> Result<()> {
    let p = plan();
    let raw = serde_json::to_vec(&p)?;
    ensure!(
        Schedule::parse(&raw, true)?.retry_live.is_some() && Schedule::parse(&raw, false).is_err(),
        "debug opt-in only"
    );
    for (key, value) in [
        ("phase", json!("observe")),
        ("phase", json!("catch_up")),
        ("lane", json!(0)),
        ("suspend_live", json!(true)),
        ("hold_ordinals", json!([16])),
    ] {
        let mut bad = p.clone();
        bad[key] = value;
        ensure!(
            Schedule::parse(&serde_json::to_vec(&bad)?, true).is_err(),
            "reject contradictory {key}"
        );
    }
    for (key, value) in [
        ("incarnation", json!(state::hex(&[0; 32]))),
        ("incarnation", json!("AB".repeat(32))),
        ("lease_generation", json!(0)),
        ("from", json!(0)),
        ("from", json!(512)),
        ("unknown", json!(true)),
    ] {
        let mut bad = p.clone();
        bad["retry_live"][key] = value;
        ensure!(
            Schedule::parse(&serde_json::to_vec(&bad)?, true).is_err(),
            "strict nested {key}"
        );
    }
    Ok(())
}
#[test]
fn s54_r2_retry_absent_is_noop_without_calling_source_reader() -> Result<()> {
    let state = Once::default();
    let calls = std::cell::Cell::new(0);
    let result = state.choose(None, [3; 32], |_| {
        calls.set(calls.get() + 1);
        anyhow::bail!("source reader must not run")
    })?;
    ensure!(
        result.is_none() && calls.get() == 0,
        "unchanged absent hook"
    );
    Ok(())
}
#[test]
fn s54_r2_completed_repeat_precedes_mutable_debt_and_frontier_checks() -> Result<()> {
    let request = request();
    let mut state = Once::default();
    let captured = state
        .choose(Some(&request), [3; 32], |r| gate().ticket(r))?
        .context("prepared actual ticket")?;
    state.begin(captured.clone())?;
    state.finish(&captured, &[[16; 32]], 17)?;
    let calls = std::cell::Cell::new(0);
    // The reader deliberately represents ACKed/missing B16 and a changed Head.
    let result = state.choose(Some(&request), [3; 32], |_| {
        calls.set(calls.get() + 1);
        anyhow::bail!("mutable debt no longer exists")
    })?;
    ensure!(
        result.is_none() && calls.get() == 0,
        "same completed token ignored before changed debt"
    );
    ensure!(
        state
            .choose(None, [3; 32], |_| anyhow::bail!("reader"))
            .is_ok(),
        "controller may clear its file later"
    );
    Ok(())
}
#[test]
fn s54_r2_consumed_token_cannot_rearm_or_move_to_new_binding() -> Result<()> {
    let ticket = ticket()?;
    let mut state = Once::default();
    state.begin(ticket.clone())?;
    state.finish(&ticket, &[[16; 32]], 17)?;
    let mut replaced = ticket.request.clone();
    replaced.from = 17;
    ensure!(
        state
            .choose(Some(&replaced), [3; 32], |_| anyhow::bail!("no rearm"))
            .is_err(),
        "replacement cannot rearm consumed lease"
    );
    ensure!(
        state
            .choose(Some(&ticket.request), [8; 32], |_| anyhow::bail!(
                "no reconnect"
            ))
            .is_err(),
        "completed token retains captured binding"
    );
    ensure!(state.begin(ticket).is_err(), "not reset by cleared file");
    Ok(())
}
#[test]
fn s54_r2_retry_rejects_stale_incarnation_generation_and_wrong_owner() -> Result<()> {
    for kind in 0..4 {
        let mut g = gate();
        match kind {
            0 => g.nonce = [9; 32],
            1 => g.generation = 8,
            2 => g.role = 0,
            _ => g.active = false,
        };
        ensure!(
            g.ticket(&request()).is_err(),
            "reject stale/wrong live owner {kind}"
        );
    }
    Ok(())
}
#[test]
fn s54_r2_retry_requires_actual_unheld_lowest_debt_at_received_frontier() -> Result<()> {
    for kind in 0..5 {
        let mut g = gate();
        match kind {
            0 => g.debt = None,
            1 => g.debt = Some((17, [17; 32])),
            2 => g.held = true,
            3 => g.received = 17,
            _ => g.normal_first = 16,
        };
        ensure!(
            g.ticket(&request()).is_err(),
            "reject guessed/held/nonlowest debt {kind}"
        );
    }
    Ok(())
}
#[test]
fn s54_r2_inflight_identity_cannot_be_replaced_or_implicitly_retried() -> Result<()> {
    let ticket = ticket()?;
    let mut state = Once::default();
    state.begin(ticket.clone())?;
    ensure!(
        state
            .choose(Some(&ticket.request), [8; 32], |r| gate().ticket(r))
            .is_err(),
        "pending RPC cannot move to a new connection"
    );
    let mut changed = ticket.clone();
    changed.binding = [8; 32];
    ensure!(
        state.finish(&changed, &[[16; 32]], 17).is_err(),
        "actual completion belongs to captured token"
    );
    state.finish(&ticket, &[[16; 32]], 17)?;
    Ok(())
}
#[test]
fn s54_r2_retry_completion_requires_original_first_id_and_wire_bound() -> Result<()> {
    let ticket = ticket()?;
    for (ids, next) in [
        (vec![], 17),
        (vec![[17; 32]], 17),
        (vec![[16; 32]; 2], 18),
        ((0..17).map(|n| [n; 32]).collect(), 33),
        (vec![[16; 32]], 16),
        (vec![[16; 32]], 33),
    ] {
        ensure!(
            completed_ids(&ticket, &ids, next).is_err(),
            "no empty/skipped/duplicate/over-cap actual retry"
        );
    }
    completed_ids(&ticket, &[[16; 32], [25; 32]], 26)?;
    Ok(())
}
