//! Supplied IPC/endpoint/lifetime inputs; no child or mixed runtime coverage.
use super::*;
#[test]
fn aq4_injected_live_sources_and_deferred_bindings() -> Result<()> {
    let f = migration_source_fixture::Fixture::new(1, [[17, 16], [25, 25]]);
    let endpoint = f.endpoints[0].as_ref().context("injected A")?;
    let source = endpoint.source;
    early_a(endpoint, source)?;
    let mut wrong = endpoint.clone();
    wrong.lanes[0].next = [25, 16];
    assert!(early_a(&wrong, source).is_err());
    assert!(early_a(endpoint, [99; 32]).is_err());
    let rows: [[Value; 2]; 2] = std::array::from_fn(|owner| {
        std::array::from_fn(|c| {
            if owner == 0 {
                json!({"context":f.contexts[c],"role":0,"phase":3,"mode":"L1","next":if c==0{[25,16]}else{[1,1]},"counts":{"queued":0,"debt":if c==0{1}else{0},"pending":if c==0{8}else{0}}})
            } else {
                json!({"context":f.contexts[c],"phase":3,"send_next":if c==0{25}else{1},"receive_next":if c==0{25}else{1},"unacked":if c==0{9}else{0},"pending":0})
            }
        })
    });
    assert!(live(&rows, &f.contexts)?); // No invented legacy queue fields.
    for case in 0..12 {
        let mut r = rows.clone();
        match case {
            0 => r[0][0]["counts"]["queued"] = 1.into(),
            1 => r[0][0]["counts"]["debt"] = 0.into(),
            2 => r[0][0]["counts"]["pending"] = 7.into(),
            3 => r[1][0]["unacked"] = 8.into(),
            4 => r[1][0]["pending"] = 1.into(),
            5 => r[1][0]["receive_next"] = 24.into(),
            6 => r[0][1]["next"] = json!([2, 1]),
            7 => r[1][1]["pending"] = 1.into(),
            8 => r[0][0]["context"] = f.contexts[1].clone().into(),
            9 => r[0][0]["role"] = 1.into(),
            10 => r[0][0]["mode"] = "Selected".into(),
            _ => r[1][1] = Value::Null,
        }
        assert!(!live(&r, &f.contexts)?, "injected status {case}");
    }
    let a = json!({"alias":{"owner":0,"current":false},"token":{"injected":true},"aborted":false,"binding":{"owner":0,"source":source}});
    let current = json!({"alias":{"owner":0,"current":true},"token":{"injected":true},"aborted":false,"binding":{"owner":0,"source":source}});
    let b = json!({"alias":{"owner":1,"current":false},"token":{"injected":true},"aborted":false,"binding":null});
    let diagnostic = json!({"actual_lifetimes":[a,current,b]});
    bindings(&diagnostic, [Some(source), None])?;
    for case in 0..6 {
        let mut d = diagnostic.clone();
        match case {
            0 => d["actual_lifetimes"][0]["binding"] = Value::Null,
            1 => d["actual_lifetimes"][1]["binding"]["source"] = json!(([99; 32])),
            2 => d["actual_lifetimes"][2]["binding"] = json!({"owner":1,"source":([32;32])}),
            3 => d["actual_lifetimes"][2]["alias"]["current"] = true.into(),
            4 => {
                d["actual_lifetimes"]
                    .as_array_mut()
                    .context("lifetimes")?
                    .pop();
            }
            _ => d["actual_lifetimes"][0]["aborted"] = true.into(),
        }
        assert!(
            bindings(&d, [Some(source), None]).is_err(),
            "injected binding {case}"
        );
    }
    let mut complete = diagnostic;
    complete["actual_lifetimes"][2]["binding"] = json!({"owner":1,"source":([32;32])});
    bindings(&complete, [Some(source), Some([32; 32])])?;
    assert!(bindings(&complete, [Some(source), Some([33; 32])]).is_err());
    println!(
        "aq4_injected_status_refusals=12 endpoint_refusals=2 memory_binding_refusals=7 actual_runtime=false"
    );
    Ok(())
}

// Actual AP Legacy packet/receipt authority; numeric metadata and pending flags
// below are supplied observations, NOT a warm L1 repository or earlier A source.
pub(in super::super) fn legacy_row_checks(
    a: &Legacy,
    b: &Legacy,
    pending: &[boundary::PendingRow],
) -> Result<usize> {
    let ctx = &a.contexts[0];
    let id = old::Event::parse(
        ctx.events.get(&(0, 24)).context("real AP A24")?,
        Spec::parse(&ctx.spec)?.peers[0].cert.device_id(),
    )?
    .id()?;
    let lanes = std::array::from_fn(|c| {
        let ctx = &a.contexts[c];
        let next = if c == 0 { [25, 16] } else { [1, 1] };
        WarmLane {
            next,
            counts: [
                ctx.events.len() as u64,
                0,
                if c == 0 { 1 } else { 0 },
                if c == 0 { 8 } else { 0 },
            ],
            lanes: next,
            projections: ctx.history.len() as u64,
            queues: ctx.queue.len(),
            debt: if c == 0 { vec![(24, id)] } else { vec![] },
            pending: if c == 0 {
                pending
                    .iter()
                    .cloned()
                    .map(|row| OrdinaryPending {
                        row,
                        head: false,
                        quarantined: false,
                    })
                    .collect()
            } else {
                vec![]
            },
            repair_pending: if c == 0 { 8 } else { 0 },
            repair_head: None,
            blocked: false,
            a24: ctx.events.get(&(0, 24)).cloned(),
            local_receipt: ctx.receipts.get(&(0, 24)).cloned(),
            peer_receipts: if c == 0 {
                (16..=24)
                    .map(|n| ctx.receipts.get(&(1, n)).cloned())
                    .collect()
            } else {
                vec![]
            },
            receipt_counts: [0, 1]
                .map(|role| ctx.receipts.keys().filter(|(r, _)| *r == role).count() as u64),
        }
    });
    let report = verify_rows(b, &lanes)?;
    ensure!(
        report["event"] == state::hex(&id) && report["ordinal"] == 24,
        "same saved-r1 report semantics"
    );
    for case in 0..18 {
        let mut wrong = lanes.clone();
        match case {
            0 => wrong[0].next = [24, 16],
            1 => wrong[0].counts[1] = 1,
            2 => wrong[0].queues = 1,
            3 => wrong[0].debt.clear(),
            4 => wrong[0].debt.push((25, id)),
            5 => wrong[0].debt[0].1 = [99; 32],
            6 => {
                wrong[0].pending.pop();
            }
            7 => wrong[0].pending.push(OrdinaryPending {
                row: pending[0].clone(),
                head: false,
                quarantined: false,
            }),
            8 => wrong[0].pending[0].head = true,
            9 => wrong[0].pending[0].quarantined = true,
            10 => wrong[0].blocked = true,
            11 => wrong[0].peer_receipts[0] = Some(vec![1]),
            12 => wrong[0].local_receipt = Some(vec![1]),
            13 => wrong[0].a24 = None,
            14 => wrong[1].counts[3] = 1,
            15 => wrong[1].lanes = [1, 2],
            16 => wrong[0].repair_head = Some(16),
            _ => wrong[0].pending[0].row.raw = pending[1].raw.clone(),
        }
        ensure!(
            verify_rows(b, &wrong).is_err(),
            "warm row supplied observation refusal {case}"
        );
    }
    println!(
        "aq4_real_AP_authority_with_injected_warm_rows_refusals=18 actual_warm_collector_positive=false"
    );
    Ok(18)
}
