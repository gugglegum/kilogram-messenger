//! G1-only live refusal bracket and conversion boundary. No new data/restarts.
use super::*;

pub(super) const FIRST_TEXT: &str = "first genuine Event2 past exhausted legacy boundary";
const CAPACITY: &str = "UpgradeRequired: send lane lifetime capacity";

fn check(until: Instant) -> Result<()> {
    s53_deadline::check()?;
    ensure!(
        Instant::now() < until,
        "G1 admission/refusal phase deadline"
    );
    Ok(())
}
fn op<T>(until: Instant, action: impl FnOnce() -> Result<T>) -> Result<T> {
    check(until)?;
    let result = action();
    check(until)?;
    result
}
fn alive(pair: &mut Pair) -> Result<[u32; 2]> {
    let mut pids = [0; 2];
    for (role, pid) in pids.iter_mut().enumerate() {
        let child = pair.children[role].as_mut().context("owned G1 runtime")?;
        child.alive()?;
        *pid = child.0.id();
    }
    Ok(pids)
}
fn statuses(pair: &mut Pair, until: Instant) -> Result<[Value; 2]> {
    op(until, || {
        alive(pair)?;
        Ok(())
    })?;
    Ok([
        op(until, || pair.nodes[0].status())?,
        op(until, || pair.nodes[1].status())?,
    ])
}
fn rows(pair: &Pair, status: &[Value; 2]) -> Result<[Value; 2]> {
    Ok([
        pair.nodes[0].context(&status[0])?.clone(),
        pair.nodes[1].context(&status[1])?.clone(),
    ])
}
fn zero_data_stats(status: &Value) -> Result<()> {
    ensure!(status["version"] == 3, "IPC3 full process status");
    for field in [
        "packet_attempts",
        "committed_encrypt",
        "committed_decrypt",
        "aborted_packet_calls",
        "ambiguous_packet_calls",
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
        "materialized",
        "readable",
    ] {
        ensure!(
            status["stats"].get(field).and_then(Value::as_u64) == Some(0),
            "G1 conversion/refusal full-process zero {field}: {}",
            status["stats"]
        );
    }
    Ok(())
}
fn baseline(row: &Value, original: &Value, role: usize) -> Result<()> {
    for field in [
        "context",
        "d",
        "e",
        "sid",
        "role",
        "next",
        "counts",
        "native_hash",
        "native_generation",
    ] {
        ensure!(
            row.get(field).is_some() && row[field] == original[field],
            "original no-data {field} role{role}"
        );
    }
    ensure!(
        row["role"] == role as u64
            && row["phase"] == 3
            && row["next"] == json!([512, 18])
            && row["native_generation"] == [517, 88][role]
            && row["native_hash"].as_str().is_some_and(|s| s.len() == 64),
        "G1 original Native/frontiers"
    );
    ensure!(
        row["counts"]["sources"] == 530
            && row["counts"]["receipts"] == json!([511, 17])
            && ["queued", "pending", "debt"]
                .iter()
                .all(|f| row["counts"][*f] == 0),
        "G1 quiescent original messages"
    );
    Ok(())
}
fn no_intent(row: &Value) -> Result<()> {
    ensure!(
        row["mode"] == "L1"
            && row["upgrade"] == "NoLocalIntent"
            && row["freeze"] == false
            && row.get("q").is_some_and(Value::is_null)
            && row["cuts"] == json!([null, null])
            && row["control_debt"] == false
            && row["native_encoding"] == "Legacy7"
            && ["p", "a", "k", "committed"]
                .iter()
                .all(|f| row["controls"].get(*f).is_some_and(Value::is_null)),
        "G1 actual L1/NoLocalIntent without Q/cuts/controls/freeze: {row}"
    );
    Ok(())
}
fn admitted(row: &Value) -> bool {
    row["lease_family"] == 2
        && row["lease_current"] == true
        && row["lease_generation"].as_u64().is_some()
        && row["publication_ready"] == true
        && row["existing_origin_ready"] == true
        && [
            "policy_error",
            "handoff_policy_error",
            "publication_error",
            "origin_publication_error",
        ]
        .iter()
        .all(|f| row.get(*f).is_some_and(Value::is_null))
}
fn consent(rows: &[Value; 2], expected: &str) -> Result<()> {
    let reference = rows[0]["current_consent2"]
        .as_str()
        .context("current Consent2 reference")?;
    ensure!(
        reference.len() == 64
            && reference.bytes().all(|b| b.is_ascii_hexdigit())
            && rows[1]["current_consent2"] == reference
            && reference == expected,
        "same legitimate current Consent2 on both roles"
    );
    Ok(())
}
fn absent(pair: &Pair, until: Instant) -> Result<Value> {
    let mut evidence = Vec::new();
    for role in 0..2 {
        let request = format!("fa00000000000000{:016x}", 600 + role);
        let raw = op(until, || {
            pair.command(
                role,
                "request-status3",
                &["--request-id".into(), request.clone()],
            )
        })?;
        let value: Value = serde_json::from_str(&raw)?;
        ensure!(
            value == json!([]),
            "G1 request{request} must remain absent: {value}"
        );
        evidence.push(json!({"role":role,"request":request,"response":value}));
    }
    Ok(json!(evidence))
}

// Existing network2 scheduler trace. The first opportunity must still be open;
// a resolved/second probe never becomes a replacement for a failed bracket.
fn first_probe(raw: &str, slot: u8) -> Result<Option<u64>> {
    ensure!(raw.len() <= 1024 * 1024, "G1 trace byte cap");
    let complete = raw.rsplit_once('\n').map_or("", |(s, _)| s);
    let mut first = None;
    for line in complete.lines() {
        let f: Vec<_> = line.split_ascii_whitespace().collect();
        if f.first().copied() != Some("s53_scheduler") {
            continue;
        }
        let actual: u8 = f
            .get(1)
            .and_then(|s| s.strip_prefix("slot="))
            .context("G1 scheduler slot")?
            .parse()?;
        if actual != slot {
            continue;
        }
        let phase = f
            .get(2)
            .and_then(|s| s.strip_prefix("phase="))
            .context("G1 scheduler phase")?;
        match phase {
            "ordinary-start" | "ordinary-finished" | "probe-start" => {
                ensure!(f.len() == 4, "G1 exact scheduler start record");
                let stamp: u64 = f[3]
                    .strip_prefix("monotonic_ms=")
                    .context("G1 scheduler clock")?
                    .parse()?;
                if phase == "probe-start" {
                    ensure!(
                        first.replace(stamp).is_none(),
                        "G1 no replacement probe opportunity"
                    );
                }
            }
            "probe-resolved" => anyhow::bail!("G1 first probe resolved before refusal/upgrade"),
            _ => anyhow::bail!("G1 unknown scheduler phase"),
        }
    }
    Ok(first)
}
fn probe(pair: &Pair) -> Result<Option<u64>> {
    let path = pair.nodes[0]
        .branch
        .base
        .join(format!("live-0-{}.stderr.log", pair.starts[0]));
    ensure!(
        std::fs::metadata(&path)?.len() <= 1024 * 1024,
        "G1 owned trace file cap"
    );
    first_probe(&std::fs::read_to_string(path)?, pair.nodes[0].slot)
}
fn quiet_end(a0: Instant, first_m: u64, setup_end: Instant) -> Result<Instant> {
    ensure!(
        first_m > 0 && u128::from(first_m) <= a0.elapsed().as_millis(),
        "G1 actual first-probe timestamp"
    );
    Ok(a0
        .checked_add(Duration::from_millis(first_m))
        .and_then(|t| t.checked_add(Duration::from_secs(15)))
        .context("G1 first opportunity deadline overflow")?
        .min(setup_end))
}
fn refusal(output: &std::process::Output) -> Result<Value> {
    let code = output.status.code().context("G1 real CLI exit code")?;
    ensure!(code != 0, "G1 send3 must exit nonzero");
    let stderr = std::str::from_utf8(&output.stderr)?.trim();
    let message = stderr.strip_prefix("Error: ").unwrap_or(stderr);
    ensure!(
        output.stdout.is_empty() && message == CAPACITY,
        "G1 exact capacity refusal, not permission/transport: {stderr}"
    );
    Ok(
        json!({"exit_code":code,"stderr":stderr,"stdout_bytes":output.stdout.len(),"request":"fa000000000000000000000000000258","text":FIRST_TEXT}),
    )
}

pub(super) struct Refused {
    pub(super) original: [Value; 2],
    pub(super) report: Value,
    until: Instant,
    first_m: u64,
    pids: [u32; 2],
}
impl Refused {
    pub(super) fn upgrade(&self, pair: &mut Pair) -> Result<()> {
        ensure!(
            op(self.until, || probe(pair))? == Some(self.first_m) && alive(pair)? == self.pids,
            "G1 same first opportunity/processes at sole upgrade"
        );
        op(self.until, || pair.upgrade(0, 61))
    }
}
pub(super) fn before_upgrade(pair: &mut Pair, cold: &[Value; 2], a0: Instant) -> Result<Refused> {
    // Public output of the just-completed legitimate ceremony; current runtime
    // permission/admission still performs every live cryptographic/time gate.
    let path = pair.base.join("exhausted.consent");
    let raw = kilogram_cli::ordered_dialog::admin::read_public(&path, 4096)?;
    let expected_consent = kilogram_cli::ordered_dialog::state::hex(
        &kilogram_protocol::ordered_dialog_v2::ParsedConsent2::parse(&raw)?.reference(),
    );
    let began = Instant::now();
    let setup_end = began + Duration::from_secs(45);
    let pids = alive(pair)?;
    op(setup_end, || pair.exchange2())?;
    let (before, initial, first_m, until) = loop {
        check(setup_end)?;
        let first = op(setup_end, || probe(pair))?;
        let until = first
            .map(|m| quiet_end(a0, m, setup_end))
            .transpose()?
            .unwrap_or(setup_end);
        check(until)?;
        let status = statuses(pair, until)?;
        let observed = rows(pair, &status)?;
        for role in 0..2 {
            baseline(&observed[role], &cold[role], role)?;
            no_intent(&observed[role])?;
            zero_data_stats(&status[role])?;
        }
        consent(&observed, &expected_consent)?;
        if observed.iter().all(admitted) {
            // A first start can be emitted between the initial trace read and
            // the status replies. Re-read that same trace, never try a new probe.
            let first_m =
                op(until, || probe(pair))?.context("G1 actual first scheduler probe-start")?;
            ensure!(first.is_none_or(|m| m == first_m), "G1 same initial probe");
            let until = quiet_end(a0, first_m, setup_end)?;
            ensure!(
                op(until, || probe(pair))? == Some(first_m),
                "G1 same first probe before bracket"
            );
            break (status, observed, first_m, until);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // Cold G1b1 bind already proves all530 sources/528 history rows use the
    // exact original request namespace (including B9001), excluding A600/B601.
    // Here public absent own indices plus unchanged counts/frontiers/Native
    // bracket the real send without expensive history or a concurrent DB open.
    let absent_before = absent(pair, until)?;
    let generations = [
        initial[0]["lease_generation"]
            .as_u64()
            .context("A local lease generation")?,
        initial[1]["lease_generation"]
            .as_u64()
            .context("B local lease generation")?,
    ];
    let output = op(until, || {
        s53_deadline::output(command(&current()).args([
            "ordered".into(),
            "send3".into(),
            "--ipc-file".into(),
            text(&pair.nodes[0].ipc),
            "--slot".into(),
            pair.nodes[0].slot.to_string(),
            "--request-id".into(),
            "fa000000000000000000000000000258".into(),
            "--text".into(),
            FIRST_TEXT.into(),
        ]))
    })?;
    let refusal = refusal(&output)?;
    let absent_after = absent(pair, until)?;
    let after = statuses(pair, until)?;
    let after_rows = rows(pair, &after)?;
    consent(&after_rows, &expected_consent)?;
    for role in 0..2 {
        baseline(&after_rows[role], &cold[role], role)?;
        no_intent(&after_rows[role])?;
        zero_data_stats(&after[role])?;
        ensure!(
            admitted(&after_rows[role])
                && after_rows[role]["lease_generation"] == generations[role]
                && after_rows[role]["current_consent2"] == initial[role]["current_consent2"],
            "G1 same current LOCAL family2 generation across refusal role{role}"
        );
        for field in [
            "native",
            "native_hash",
            "native_generation",
            "native_encoding",
            "next",
            "counts",
            "upgrade",
            "q",
            "cuts",
            "freeze",
            "control_debt",
            "controls",
        ] {
            ensure!(
                initial[role].get(field).is_some()
                    && initial[role][field] == after_rows[role][field],
                "G1 unchanged refusal message/Native {field} role{role}"
            );
        }
        ensure!(
            after_rows[role]["native"]["sid"] == after_rows[role]["sid"]
                && after_rows[role]["native"]["keys"]
                    .as_array()
                    .is_some_and(|keys| keys.len() == 3
                        && keys
                            .iter()
                            .all(|key| key.as_str().is_some_and(|s| s.len() == 64))),
            "G1 actual original Native public keys"
        );
    }
    ensure!(
        op(until, || probe(pair))? == Some(first_m) && alive(pair)? == pids,
        "G1 same processes/first opportunity through refusal"
    );
    check(until)?;
    // No intervening work after this return: caller issues the sole upgrade.
    Ok(Refused {
        original: after_rows,
        until,
        first_m,
        pids,
        report: json!({"before":before,"after":after,"pids":pids,
        "local_lease_generations":generations,"first_probe_ms":first_m,"setup_seconds":45,"quiet_seconds":15,
        "refusal":refusal,"request_absence_before":absent_before,"request_absence_after":absent_after,
        "elapsed_seconds":began.elapsed().as_secs_f64(),"original_source_history_absence":"G1b1 exact530-source/528-row original namespace binding before admission; no new source/queue/Native change in bracket"}),
    })
}

pub(super) fn converted(pair: &mut Pair, refused: &Refused) -> Result<Value> {
    let original = &refused.original;
    let consent2 = &original[0]["current_consent2"];
    let status = statuses(pair, Instant::now() + Duration::from_secs(15))?;
    let current_rows = rows(pair, &status)?;
    for role in 0..2 {
        let row = &current_rows[role];
        same_origin(&original[role], row)?;
        ensure!(
            row["role"] == role as u64
                && row["mode"] == "Active"
                && row["cuts"] == json!([512, 18])
                && row["next"] == json!([512, 18])
                && row["native_encoding"] == "Mixed7"
                && row["native_generation"] == [518, 89][role]
                && row["current_consent2"] == *consent2
                && row["counts"] == original[role]["counts"]
                && row["control_debt"] == false
                && row.get("policy_error").is_some_and(Value::is_null)
                && row.get("handoff_policy_error").is_some_and(Value::is_null)
                && row["q"].as_str().is_some_and(|s| s.len() == 64),
            "G1 conversion-only boundary before new data role{role}: {row}"
        );
        zero_data_stats(&status[role])?;
    }
    ensure!(
        current_rows[0]["q"] == current_rows[1]["q"],
        "G1 same Active selection Q"
    );
    ensure!(
        alive(pair)? == refused.pids,
        "G1 same processes at conversion-only boundary"
    );
    Ok(
        json!({"status":status,"pids":alive(pair)?,"cuts":[512,18],"native_generations":[518,89],"before_any_new_data_send":true}),
    )
}

#[test]
fn g1b2_real_cli_failure_and_scheduler_trace_parsers() -> Result<()> {
    use std::os::windows::process::ExitStatusExt;
    let result = |code, message: &str| std::process::Output {
        status: std::process::ExitStatus::from_raw(code),
        stdout: vec![],
        stderr: message.as_bytes().to_vec(),
    };
    refusal(&result(1, &format!("Error: {CAPACITY}\n")))?;
    ensure!(
        refusal(&result(0, &format!("Error: {CAPACITY}\n"))).is_err(),
        "success exit not refusal"
    );
    for message in [
        "Error: AuthBlocked: current account pin",
        "Error: IPC connection refused",
        "Error: context: UpgradeRequired: send lane lifetime capacity",
    ] {
        ensure!(
            refusal(&result(1, message)).is_err(),
            "permission/transport/substring is not exact capacity refusal"
        );
    }
    let trace = "s53_scheduler slot=0 phase=ordinary-start monotonic_ms=1\ns53_scheduler slot=0 phase=ordinary-finished monotonic_ms=2\ns53_scheduler slot=0 phase=probe-start monotonic_ms=3\n";
    ensure!(
        first_probe(trace, 0)? == Some(3),
        "actual emitted scheduler line format"
    );
    ensure!(
        first_probe(trace, 1)?.is_none(),
        "another slot is not this opportunity"
    );
    for suffix in [
        "s53_scheduler slot=0 phase=probe-resolved mode=L1 monotonic_ms=4\n",
        "s53_scheduler slot=0 phase=probe-start monotonic_ms=5\n",
        "s53_scheduler slot=0 phase=probe-start monotonic_ms=bad\n",
    ] {
        ensure!(
            first_probe(&format!("{trace}{suffix}"), 0).is_err(),
            "resolved/replacement/malformed probe refused"
        );
    }
    ensure!(
        first_probe(
            &format!("{trace}s53_scheduler slot=0 phase=probe-resolved"),
            0
        )? == Some(3),
        "incomplete final trace write is not a complete resolution"
    );
    Ok(())
}
