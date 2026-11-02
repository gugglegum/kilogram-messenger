//! Only pure metadata, command configuration and fresh tiny files. No product child.
use super::*;
fn counts(e: u64, d: u64) -> Value {
    json!({"encrypt":e,"decrypt":d,"attempts":e+d,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0})
}
fn event(kind: &str, seq: u64, facts: Value) -> String {
    format!(
        "s54_observation {}",
        json!({"kind":kind,"pid":73,"owner":state::hex(&[1;32]),"incarnation":state::hex(&[2;32]),"sequence":seq,"facts":facts})
    )
}
fn decoder() -> observations::Decoder {
    observations::Decoder::new(73, [1; 32], [state::hex(&[3; 32]), state::hex(&[4; 32])], 0)
}
fn native(seq: u64, generation: u64, e: u64) -> String {
    event(
        "native_commit",
        seq,
        json!({"context":state::hex(&[3;32]),"operation":state::hex(&[generation as u8;32]),"generation":generation,"counters":counts(e,0)}),
    )
}
fn start(
    d: &mut observations::Decoder,
    o: &mut oracle::Operations,
    p: &mut oracle::Pages,
) -> Result<()> {
    d.line(
        &event("start", 0, json!({"counters":counts(0,0)})),
        o,
        p,
        &mut 0,
    )
}

#[test]
fn s54_b2_actual_envelope_start_commit_pending_head_and_final_decode() -> Result<()> {
    let mut d = decoder();
    let mut o = Default::default();
    let mut p = Default::default();
    let mut heads = 0;
    start(&mut d, &mut o, &mut p)?;
    d.line(&native(1, 7, 2), &mut o, &mut p, &mut heads)?;
    d.line(&event("saved_pending",1,json!({"context":state::hex(&[3;32]),"role":1,"ordinal":25,"event":state::hex(&[9;32]),"generation":8,"operation":state::hex(&[8;32])})),&mut o,&mut p,&mut heads)?;
    d.line(&event("validated_head",1,json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"pre_q":true,"received":16})),&mut o,&mut p,&mut heads)?;
    ensure!(
        heads == 1 && d.pending.contains(&[9; 32]),
        "actual pending and pre-Q witnesses wired"
    );
    ensure!(
        d.line(
            &event("final", 0, json!({"counters":counts(2,0),"resolved":true})),
            &mut o,
            &mut p,
            &mut heads
        )
        .is_err(),
        "final sequence cannot be stale"
    );
    d.line(
        &event("final", 1, json!({"counters":counts(2,0),"resolved":true})),
        &mut o,
        &mut p,
        &mut heads,
    )?;
    ensure!(o.complete()? == (2, 0), "observed ledger total");
    Ok(())
}
#[test]
fn s54_b2_decoder_rejects_pid_owner_nonce_context_and_unknown_kind() -> Result<()> {
    for change in ["pid", "owner", "incarnation", "context", "kind"] {
        let mut d = decoder();
        let mut o = Default::default();
        let mut p = Default::default();
        start(&mut d, &mut o, &mut p)?;
        let mut v: Value = serde_json::from_str(
            native(1, 7, 1)
                .strip_prefix("s54_observation ")
                .context("test envelope prefix")?,
        )?;
        match change {
            "pid" => v["pid"] = 74.into(),
            "owner" => v["owner"] = state::hex(&[6; 32]).into(),
            "incarnation" => v["incarnation"] = state::hex(&[6; 32]).into(),
            "context" => v["facts"]["context"] = state::hex(&[6; 32]).into(),
            _ => v["kind"] = "unrecognized".into(),
        }
        ensure!(
            d.line(&format!("s54_observation {v}"), &mut o, &mut p, &mut 0)
                .is_err(),
            "reject {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_b2_decoder_rejects_missing_start_sequence_and_counter_regression() -> Result<()> {
    let mut d = decoder();
    let mut o = Default::default();
    let mut p = Default::default();
    ensure!(
        d.line(&native(1, 7, 1), &mut o, &mut p, &mut 0).is_err(),
        "no fabricated start"
    );
    start(&mut d, &mut o, &mut p)?;
    ensure!(
        d.line(&native(2, 7, 1), &mut o, &mut p, &mut 0).is_err(),
        "missing sequence"
    );
    d.line(&native(1, 7, 2), &mut o, &mut p, &mut 0)?;
    ensure!(
        d.line(&native(2, 8, 1), &mut o, &mut p, &mut 0).is_err(),
        "regressing actual sample"
    );
    Ok(())
}
#[test]
fn s54_b2_start_overlap_and_unresolved_final_cannot_close_ledger() -> Result<()> {
    let mut d = decoder();
    let mut e = decoder();
    let mut o = Default::default();
    let mut p = Default::default();
    start(&mut d, &mut o, &mut p)?;
    ensure!(
        start(&mut e, &mut o, &mut p).is_err(),
        "overlap/reused incarnation rejected"
    );
    ensure!(
        d.line(
            &event("final", 0, json!({"counters":counts(0,0),"resolved":false})),
            &mut o,
            &mut p,
            &mut 0
        )
        .is_err(),
        "explicit unresolved repository"
    );
    ensure!(o.complete().is_err(), "missing final remains unresolved");
    Ok(())
}
#[test]
fn s54_b2_kill_outcome_requires_stopped_sources_and_actual_resolved_counters() -> Result<()> {
    for bad in [false, true] {
        let mut d = decoder();
        let mut o = Default::default();
        let mut p = Default::default();
        start(&mut d, &mut o, &mut p)?;
        d.line(&native(1, 7, 2), &mut o, &mut p, &mut 0)?;
        let end = source::End {
            point: if bad { (13, 20) } else { (12, 20) },
            endpoint: intervals::Endpoint {
                source: [1; 32],
                lanes: [
                    intervals::Lane {
                        context: [3; 32],
                        slot: 0,
                        role: 0,
                        next: [if bad { 14 } else { 13 }, 21],
                    },
                    intervals::Lane {
                        context: [4; 32],
                        slot: 1,
                        role: 0,
                        next: [1, 1],
                    },
                ],
            },
            generation: 8,
            operation: [8; 32],
        };
        let actual = oracle::Counts::read(&counts(2, 0))?;
        let result = d.close_killed(&mut o, (10, 20), end, actual);
        ensure!(
            result.is_ok() != bad,
            "stopped durable sources must agree, no guessed tail"
        );
        if !bad {
            ensure!(o.complete()? == (2, 0), "source-bound process closure");
        }
    }
    Ok(())
}
#[test]
fn s54_b2_incremental_log_carries_fragment_and_rejects_truncation() -> Result<()> {
    use std::io::Write;
    let parent = workspace().join(".tmp");
    let temp = tempfile::Builder::new()
        .prefix("s54-b2-decoder-")
        .tempdir_in(parent)?;
    let path = temp.path().join("own.log");
    let line = event("start", 0, json!({"counters":counts(0,0)}));
    std::fs::write(&path, &line.as_bytes()[..17])?;
    let mut d = decoder();
    let mut o = Default::default();
    let mut p = Default::default();
    d.poll(&path, &mut o, &mut p, &mut 0)?;
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&path)?;
        f.write_all(&line.as_bytes()[17..])?;
        f.write_all(b"\n")?;
    }
    d.poll(&path, &mut o, &mut p, &mut 0)?;
    std::fs::write(&path, b"")?;
    ensure!(
        d.poll(&path, &mut o, &mut p, &mut 0).is_err(),
        "no log rewind/reset"
    );
    Ok(())
}
#[test]
fn s54_b2_per_child_controls_keep_frozen_schema_and_remove_inherited_hooks() -> Result<()> {
    let temp = tempfile::Builder::new()
        .prefix("s54-b2-controls-")
        .tempdir_in(workspace().join(".tmp"))?;
    let controls = control::Controls::new(temp.path(), &state::hex(&[3; 32]), 1)?;
    for indexed in [false, true] {
        let mut cmd = command(Path::new("must-not-execute.exe"));
        controls.attach(&mut cmd, indexed);
        let env = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|p| p.to_os_string()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        ensure!(
            env.get("KILOGRAM_TEST_S54_SCHEDULE")
                == Some(&indexed.then(|| controls.s54.as_os_str().to_os_string())),
            "only current child gets S54 control"
        );
        ensure!(
            env.get("KILOGRAM_TEST_OD1_CLOCK_FILE") == Some(&None),
            "inherited clock removed"
        );
    }
    let old: Value = serde_json::from_slice(&std::fs::read(&controls.od1)?)?;
    ensure!(
        old.get("phase").is_none()
            && old.get("hold_source64").is_none()
            && old["hold_ordinals"] == json!([16]),
        "frozen receives only old fields"
    );
    Ok(())
}
fn row(n: u8) -> Value {
    json!({"format":1,"id":state::hex(&[n;32]),"role":0,"ordinal":n,"request":format!("{n:032x}"),"text":format!("actual-row-{n}")})
}
fn first() -> Value {
    json!({"version":3,"items":(1..=16).map(row).collect::<Vec<_>>(),"view":state::hex(&[8;32]),"F":[25,16],"cursor":"opaque-authenticated-by-repository","finished":false})
}
fn suffix() -> Value {
    json!({"version":3,"items":(17..=39).map(row).collect::<Vec<_>>(),"view":state::hex(&[8;32]),"F":[25,16],"cursor":"opaque-terminal","finished":true})
}
#[test]
fn s54_b2_frozen_cursor_calls_reader_and_compares_actual_ordered_suffix() -> Result<()> {
    let mut reads = 0;
    let frozen = early::Frozen::from_pages(&first(), |cursor| {
        reads += 1;
        ensure!(
            cursor == "opaque-authenticated-by-repository",
            "actual first cursor passed"
        );
        Ok(suffix())
    })?;
    frozen.verify_with(|_| {
        reads += 1;
        Ok(suffix())
    })?;
    ensure!(reads == 2, "verification performs a fresh actual read");
    let mut changed = suffix();
    changed["items"]
        .as_array_mut()
        .context("test rows")?
        .swap(0, 1);
    ensure!(
        frozen.verify_with(|_| Ok(changed.clone())).is_err(),
        "copied expected list cannot hide changed actual order"
    );
    Ok(())
}
#[test]
fn s54_b2_frozen_cursor_rejects_wrong_view_frontier_or_source() -> Result<()> {
    let frozen = early::Frozen::from_pages(&first(), |_| Ok(suffix()))?;
    for key in ["view", "F", "items"] {
        let mut actual = suffix();
        match key {
            "view" => actual["view"] = "other".into(),
            "F" => actual["F"] = json!([641, 641]),
            _ => actual["items"][0]["id"] = state::hex(&[0; 32]).into(),
        }
        ensure!(
            frozen.verify_with(|_| Ok(actual.clone())).is_err(),
            "wrong actual {key}"
        );
    }
    Ok(())
}
#[test]
fn s54_b2_current_lease_bracket_rejects_reconnect_role_or_mode_substitution() -> Result<()> {
    let rows = std::array::from_fn(
        |role| json!({"role":role,"mode":if role==0{"Selected"}else{"Active"},"lease_current":true,"lease_family":2,"lease_generation":10+role,"policy_error":null,"handoff_policy_error":null}),
    );
    let generations = transition::lease_rows(&rows, ["Selected", "Active"], None)?;
    for field in ["lease_generation", "role", "mode", "lease_current"] {
        let mut bad = rows.clone();
        bad[0][field] = match field {
            "mode" => "Active".into(),
            "lease_current" => false.into(),
            _ => 99.into(),
        };
        ensure!(
            transition::lease_rows(&bad, ["Selected", "Active"], Some(generations)).is_err(),
            "reject changed {field}"
        );
    }
    Ok(())
}
#[test]
fn s54_b2_stats_decode_real_fields_and_reject_missing_or_constructor_attempts() -> Result<()> {
    let status = json!({"committed_encrypt":2,"committed_decrypt":1,"packet_attempts":3,"aborted_packet_calls":0,"ambiguous_packet_calls":0,"account_constructors":0,"outbound_constructors":0,"inbound_constructors":0});
    ensure!(
        observations::stats(&status)?.resolved(),
        "actual status fields"
    );
    let mut missing = status.clone();
    missing
        .as_object_mut()
        .context("test stats")?
        .remove("packet_attempts");
    ensure!(
        observations::stats(&missing).is_err(),
        "do not synthesize attempts from successes"
    );
    Ok(())
}
#[test]
fn s54_b2_original_request_ids_and_history_reject_alias_and_duplicate_source() -> Result<()> {
    let a = oracle::Request::new(0, 0, 25)?;
    let b = oracle::Request::new(0, 1, 25)?;
    ensure!(
        source::request_id(a) != source::request_id(b)
            && source::parse_request_id(&source::request_id(b))?.len() == 16,
        "actual request encoding preserves role"
    );
    ensure!(
        source::history_map(&[row(1), row(1)]).is_err(),
        "duplicate actual source cannot satisfy a cardinality"
    );
    ensure!(
        source::parse_request_id("fffffffffffffffffffffffffffffffZ").is_err(),
        "canonical request"
    );
    Ok(())
}
#[test]
fn s54_b2_publication_workers_fail_before_any_origin_or_child() -> Result<()> {
    ensure!(
        control::runtime_workers(1, None).is_err()
            && control::runtime_workers(8, Some("1")).is_err()
            && control::runtime_workers(8, Some("invalid")).is_err(),
        "fail closed if an existing blocking publication hook would starve its owner"
    );
    ensure!(
        control::runtime_workers(8, Some("2"))? == 2 && control::runtime_workers(4, None)? == 4,
        "explicit supported worker setting"
    );
    Ok(())
}
fn retry_decoder() -> observations::Decoder {
    observations::Decoder::new(73, [1; 32], [state::hex(&[3; 32]), state::hex(&[4; 32])], 1)
}
fn retry_head() -> String {
    event(
        "validated_head",
        0,
        json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"pre_q":false,"received":16}),
    )
}
fn retry_facts() -> Value {
    json!({"context":state::hex(&[3;32]),"binding":state::hex(&[5;32]),"lease_generation":6,"from":16,"normal_first":26,"first_id":state::hex(&[16;32]),"ids":[state::hex(&[16;32])],"next":17})
}
#[test]
fn s54_r2_decoder_binds_retry_to_actual_start_head_and_source_without_native_effect() -> Result<()>
{
    let mut d = retry_decoder();
    let mut o = Default::default();
    let mut p = Default::default();
    start(&mut d, &mut o, &mut p)?;
    d.line(&retry_head(), &mut o, &mut p, &mut 0)?;
    let request = d.expect_live_retry(6, 16, [16; 32])?;
    ensure!(
        request["incarnation"] == state::hex(&[2; 32]),
        "request derives from actual decoded start nonce"
    );
    d.line(
        &event("live_retry_complete", 0, retry_facts()),
        &mut o,
        &mut p,
        &mut 0,
    )?;
    ensure!(
        d.live_retry_complete.is_some(),
        "actual possession observed separately"
    );
    d.line(
        &event("final", 0, json!({"counters":counts(0,0),"resolved":true})),
        &mut o,
        &mut p,
        &mut 0,
    )?;
    ensure!(
        o.complete()? == (0, 0),
        "retransmission is not a new Native call"
    );
    Ok(())
}
#[test]
fn s54_r2_decoder_rejects_unarmed_stale_or_wrong_retry_complete() -> Result<()> {
    for kind in 0..5 {
        let mut d = retry_decoder();
        let mut o = Default::default();
        let mut p = Default::default();
        start(&mut d, &mut o, &mut p)?;
        d.line(&retry_head(), &mut o, &mut p, &mut 0)?;
        if kind > 0 {
            d.expect_live_retry(6, 16, [16; 32])?;
        }
        let mut facts = retry_facts();
        match kind {
            1 => facts["lease_generation"] = 7.into(),
            2 => facts["ids"] = json!([state::hex(&[17; 32])]),
            3 => facts["binding"] = state::hex(&[9; 32]).into(),
            4 => facts["counters"] = counts(1, 0),
            _ => {}
        }
        ensure!(
            d.line(
                &event("live_retry_complete", 0, facts),
                &mut o,
                &mut p,
                &mut 0
            )
            .is_err(),
            "reject unarmed/stale/contradictory retry {kind}"
        );
    }
    Ok(())
}
#[test]
fn s54_r2_both_waits_gate_source_reads_and_one_absolute_early_bound() -> Result<()> {
    for (running, stopped) in [
        ([false, true], [true, false]),
        ([true, false], [false, true]),
        ([false, false], [true, false]),
    ] {
        ensure!(
            control::waited_pair(running, stopped).is_err(),
            "cannot reconcile before both waits"
        );
    }
    control::waited_pair([false, false], [true, true])?;
    let start = Instant::now();
    let whole = start + Duration::from_secs(8);
    ensure!(
        control::interval10(start, whole) == whole
            && control::interval10(start, start + Duration::from_secs(100))
                == start + Duration::from_secs(10),
        "causal trigger and whole bound fixed, no marker-renewal"
    );
    Ok(())
}
