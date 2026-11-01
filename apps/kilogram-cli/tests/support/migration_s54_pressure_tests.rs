//! Focused P8 pure witnesses/decoder negatives. No product child or private source.
use super::*;
#[test]
fn s54_p8_c2_permission_uses_original_consent1_without_requiring_future_consent2() -> Result<()> {
    let original = json!(state::hex(&[7; 32]));
    let row = json!({"mode":"L1","alpn1_allowed":true,"lease_current":true,"lease_family":1,"policy_error":null,"current_consent":original,"current_consent2":null,"handoff_policy_error":"current Consent2 absent"});
    consent1(&row, &original)?;
    for key in [
        "policy_error",
        "current_consent",
        "current_consent2",
        "mode",
        "lease_family",
    ] {
        let mut bad = row.clone();
        bad[key] = match key {
            "lease_family" => 2.into(),
            "mode" => "Selected".into(),
            _ => state::hex(&[8; 32]).into(),
        };
        ensure!(
            consent1(&bad, &original).is_err(),
            "current C2 permission/scope failure {key}"
        );
    }
    Ok(())
}
#[test]
fn s54_p8_c2_history_rejects_wrong_format_body_alias_and_missing_source() -> Result<()> {
    let mut rows = Vec::new();
    for role in 0..2 {
        for n in 1..=32 {
            let request = oracle::Request::new(1, role, n)?;
            let id = old::hash(b"pure-test-id", &request.numeric().to_be_bytes());
            rows.push(json!({"format":1,"id":state::hex(&id),"role":role,"ordinal":n,"request":source::request_id(request),"text":request.body()}));
        }
    }
    history(&rows)?;
    for key in ["format", "text", "request", "missing"] {
        let mut bad = rows.clone();
        match key {
            "format" => bad[0][key] = 2.into(),
            "text" => bad[0][key] = "other".into(),
            "request" => bad[0][key] = bad[1][key].clone(),
            _ => {
                bad.pop();
            }
        }
        ensure!(history(&bad).is_err(), "reject C2 history {key}");
    }
    Ok(())
}
#[test]
fn s54_p8_native_batch_observation_uses_actual_per_commit_deltas() -> Result<()> {
    let (mut d, mut o, mut p) = decoder(0)?;
    for (sequence, total, peak) in [(1u64, 4u64, 4u64), (2, 7, 4), (3, 12, 5)] {
        let mut counts = zero();
        counts["encrypt"] = total.into();
        counts["attempts"] = total.into();
        let event = envelope(
            "native_commit",
            sequence,
            json!({"context":state::hex(&[4;32]),"generation":sequence+10,"operation":state::hex(&[sequence as u8;32]),"counters":counts}),
        );
        let result = d.line(&event, &mut o, &mut p, &mut 0);
        if peak > 4 {
            ensure!(
                result.is_err() && d.max_packet_delta == 4,
                "existing ledger rejects oversize batch before observation is accepted"
            );
            continue;
        }
        result?;
        ensure!(
            d.max_packet_delta == peak,
            "observed batch is a delta, not cumulative counter"
        );
    }
    Ok(())
}
fn vector() -> Vec<(u64, old::Id)> {
    (385..=640)
        .map(|n| {
            let mut id = [0; 32];
            id[..8].copy_from_slice(&u64::to_be_bytes(n));
            (n, id)
        })
        .collect()
}
fn envelope(kind: &str, seq: u64, facts: Value) -> String {
    format!(
        "s54_observation {}",
        json!({"kind":kind,"pid":73,"owner":state::hex(&[1;32]),"incarnation":state::hex(&[2;32]),"sequence":seq,"facts":facts})
    )
}
fn zero() -> Value {
    json!({"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0})
}
fn line(
    d: &mut observations::Decoder,
    o: &mut oracle::Operations,
    p: &mut oracle::Pages,
    kind: &str,
    f: Value,
) -> Result<()> {
    d.line(&envelope(kind, 0, f), o, p, &mut 0)
}
fn head() -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"pre_q":false,"received":385})
}
fn decoder(role: u8) -> Result<(observations::Decoder, oracle::Operations, oracle::Pages)> {
    let mut d = observations::Decoder::new(
        73,
        [1; 32],
        [state::hex(&[3; 32]), state::hex(&[4; 32])],
        role,
    );
    let mut o = Default::default();
    let mut p = Default::default();
    line(&mut d, &mut o, &mut p, "start", json!({"counters":zero()}))?;
    line(&mut d, &mut o, &mut p, "validated_head", head())?;
    Ok((d, o, p))
}
fn selection(ids: &[(u64, old::Id)]) -> Result<Value> {
    Ok(
        json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"after":385,"first":386,"last":449,"count":64,"ids_digest":blake3::hash(&serde_json::to_vec(ids)?).to_hex().to_string()}),
    )
}
#[test]
fn s54_p8_original_successors_and_pending_require_exact_real_associations() -> Result<()> {
    let all = vector();
    let ids = successors(&all)?;
    let mut rows = BTreeMap::new();
    for (n, id) in &ids {
        rows.insert(
            (0, 0, *n),
            observations::Pending {
                context: 0,
                role: 0,
                ordinal: *n,
                event: *id,
                generation: 10,
                operation: [9; 32],
            },
        );
    }
    pending(&rows, &ids)?;
    rows.get_mut(&(0, 0, 400)).context("test pending")?.event = [9; 32];
    ensure!(
        pending(&rows, &ids).is_err(),
        "cardinality alone cannot hide wrong actual source"
    );
    let mut wrong = all;
    wrong.swap(0, 1);
    ensure!(
        successors(&wrong).is_err(),
        "no replacement/disordered P7 vector"
    );
    Ok(())
}
#[test]
fn s54_p8_source_pause_requires_armed_current_head_and_actual_digest() -> Result<()> {
    let ids = successors(&vector())?;
    for key in [
        "unarmed",
        "after",
        "binding",
        "lease_generation",
        "first",
        "last",
        "count",
        "ids_digest",
    ] {
        let (mut d, mut o, mut p) = decoder(0)?;
        if key != "unarmed" {
            d.expect_source64(385, &ids, [5; 32], 6)?;
        }
        let mut f = selection(&ids)?;
        if key != "unarmed" {
            f[key] = if matches!(key, "binding" | "ids_digest") {
                state::hex(&[8; 32]).into()
            } else {
                999.into()
            };
        }
        ensure!(
            line(&mut d, &mut o, &mut p, "source64", f).is_err(),
            "reject source64 {key}"
        );
    }
    let (mut d, mut o, mut p) = decoder(0)?;
    d.expect_source64(385, &ids, [5; 32], 6)?;
    let f = selection(&ids)?;
    line(&mut d, &mut o, &mut p, "source64", f.clone())?;
    let mut marker: Value = serde_json::from_str(
        envelope("source64", 0, f)
            .strip_prefix("s54_observation ")
            .context("prefix")?,
    )?;
    d.marker(&marker)?;
    marker["incarnation"] = state::hex(&[8; 32]).into();
    ensure!(
        d.marker(&marker).is_err(),
        "old marker cannot substitute a live hold"
    );
    line(
        &mut d,
        &mut o,
        &mut p,
        "final",
        json!({"counters":zero(),"resolved":true}),
    )?;
    ensure!(o.complete()? == (0, 0), "source refs are not Native calls");
    Ok(())
}
#[test]
fn s54_p8_head_observation_rejects_missing_frontier_stale_binding_and_generation() -> Result<()> {
    for key in ["received", "binding", "lease_generation"] {
        let (mut d, mut o, mut p) = decoder(0)?;
        let mut f = head();
        match key {
            "received" => {
                f.as_object_mut().context("object")?.remove(key);
            }
            "binding" => f[key] = state::hex(&[9; 32]).into(),
            _ => f[key] = 5.into(),
        }
        ensure!(
            line(&mut d, &mut o, &mut p, "validated_head", f).is_err(),
            "reject stale/missing Head {key}"
        );
        ensure!(
            d.expect_source64(386, &successors(&vector())?, [5; 32], 6)
                .is_err(),
            "configured frontier cannot replace accepted385"
        );
    }
    Ok(())
}
fn page() -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"local_page":1,"head":false,"grant":null,"start":450,"end":466,"new_sources":16,"generation":10,"operation":state::hex(&[9;32])})
}
#[test]
fn s54_p8_page_barrier_matches_prior_forward_page_without_double_count() -> Result<()> {
    let (mut d, mut o, mut p) = decoder(1)?;
    let f = page();
    ensure!(
        line(&mut d, &mut o, &mut p, "page", f.clone()).is_err(),
        "no unseen page barrier"
    );
    line(&mut d, &mut o, &mut p, "normal_page", f.clone())?;
    let key = oracle::Incarnation {
        pid: 73,
        owner: [1; 32],
        nonce: [2; 32],
    };
    let target = p.local_target(key, 2)?;
    let mut wrong = f.clone();
    wrong["operation"] = state::hex(&[8; 32]).into();
    ensure!(
        line(&mut d, &mut o, &mut p, "page", wrong).is_err(),
        "wrong actual operation"
    );
    line(&mut d, &mut o, &mut p, "page", f.clone())?;
    ensure!(
        p.local_target(key, 2)? == target,
        "barrier is not a second accepted page"
    );
    ensure!(
        line(&mut d, &mut o, &mut p, "page", f).is_err(),
        "no duplicate barrier"
    );
    Ok(())
}
#[test]
fn s54_p8_pending_decoder_rejects_alias_wrong_role_and_stale_commit() -> Result<()> {
    let (mut d, mut o, mut p) = decoder(1)?;
    let f = json!({"context":state::hex(&[3;32]),"role":0,"ordinal":386,"event":state::hex(&[9;32]),"generation":10,"operation":state::hex(&[8;32])});
    line(&mut d, &mut o, &mut p, "saved_pending", f.clone())?;
    line(&mut d, &mut o, &mut p, "saved_pending", f.clone())?;
    for key in ["role", "event", "generation", "context"] {
        let mut bad = f.clone();
        bad[key] = if key == "event" || key == "context" {
            state::hex(&[7; 32]).into()
        } else {
            1.into()
        };
        ensure!(
            line(&mut d, &mut o, &mut p, "saved_pending", bad).is_err(),
            "pending alias {key}"
        );
    }
    let mut stale = f;
    stale["ordinal"] = 387.into();
    stale["event"] = state::hex(&[7; 32]).into();
    stale["generation"] = 9.into();
    ensure!(
        line(&mut d, &mut o, &mut p, "saved_pending", stale).is_err(),
        "stale durable generation"
    );
    Ok(())
}
#[test]
fn s54_p8_ack_pause_is_nonempty_bound_and_not_retirement_or_native() -> Result<()> {
    for reply in [false, true] {
        let (mut d, mut o, mut p) = decoder(0)?;
        let f = json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"request":"01010101010101010101010101010101","query_reply":reply,"receipt_count":1,"saved_receipts_digest":state::hex(&[9;32]),"lane":1,"hold_ack_from":null,"receipts":[[2,2,1,640,state::hex(&[9;32]),state::hex(&[8;32])]]});
        line(&mut d, &mut o, &mut p, "ack", f.clone())?;
        for key in [
            "receipt_count",
            "lease_generation",
            "query_reply",
            "request",
        ] {
            let mut bad = f.clone();
            bad[key] = match key {
                "request" => "bad".into(),
                "query_reply" => Value::Null,
                _ => 0.into(),
            };
            ensure!(
                line(&mut d, &mut o, &mut p, "ack", bad).is_err(),
                "reject ACK {key}"
            );
        }
        line(
            &mut d,
            &mut o,
            &mut p,
            "final",
            json!({"counters":zero(),"resolved":true}),
        )?;
        ensure!(
            o.complete()? == (0, 0),
            "ACK pause adds no Native/retirement"
        );
    }
    Ok(())
}
#[test]
fn s54_p8_context_request_path_preserves_c1_and_binds_c2_role_and_limit() -> Result<()> {
    for owner in 0..2 {
        let a = control::request(owner, 0, 640)?;
        let b = control::request(owner, 1, 32)?;
        ensure!(
            a == oracle::Request::new(0, owner as u8, 640)?
                && b.context == 1
                && b.role == owner as u8
                && source::request_id(a) != source::request_id(b),
            "same C1 mapping and distinct C2 originals"
        );
    }
    ensure!(
        control::request(0, 1, 33).is_err()
            && control::request(2, 1, 1).is_err()
            && control::request(0, 2, 1).is_err(),
        "no context/role/ordinal substitution"
    );
    Ok(())
}
#[test]
fn s54_p8_resource_witness_uses_physical_c1_slot_and_real_bounds() -> Result<()> {
    let f = json!({"window":[[64,64],[0,64],[32,64]],"resources":{"connections":2,"peak_connections":2,"mailbox":0,"peak_mailbox":4,"connection_limit":2,"mailbox_limit":16,"frame_limit":1048576,"bidi_limit":4,"uni_limit":0}});
    resources(&f, 1, true)?;
    ensure!(
        resources(&f, 0, true).is_err(),
        "logical C1 is not a physical slot assumption"
    );
    for key in ["peak_connections", "peak_mailbox", "frame_limit"] {
        let mut bad = f.clone();
        bad["resources"][key] = 9999999.into();
        ensure!(
            resources(&bad, 1, true).is_err(),
            "reject real limit breach {key}"
        );
    }
    Ok(())
}
