//! A3 memory amendment1: signed, same-point comparisons of both warm phases.
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MIB: u64 = 1024 * 1024;
const PHASES: [&str; 6] = [
    "before_fill",
    "after_fill",
    "after_reopen",
    "first_page_live",
    "after_warm_range_no_expected_vector",
    "after_ordinary_append",
];

pub(super) fn measurement_available(s: &Value) -> bool {
    s["process_memory_available"].as_bool() == Some(true)
        && s["cache_metrics_available"].as_bool() == Some(true)
        && s["process"]["private_bytes"].as_u64().is_some()
        && s["redb_cache_data_bytes"].as_u64().is_some()
}

fn point(stage: &Value, phase: &str) -> Option<(i128, i128, u64)> {
    let s = stage["memory_phases"]
        .as_array()?
        .iter()
        .find(|s| s["phase"] == phase)?;
    if !measurement_available(s) {
        return None;
    }
    Some((
        i128::from(s["process"]["private_bytes"].as_u64()?),
        i128::from(s["redb_cache_data_bytes"].as_u64()?),
        s["records"].as_u64()?,
    ))
}

pub(super) fn evaluate(stages: &[Value], elapsed: f64) -> Value {
    let mut failures = Vec::<String>::new();
    let mut unavailable = Vec::<String>::new();
    let mut processes = BTreeSet::new();
    let mut max_memory: Option<u64> = None;
    let mut max_cache: Option<u64> = None;
    let targets: Vec<_> = stages
        .iter()
        .filter_map(|s| s["target_records"].as_u64())
        .collect();
    if targets != [1000, 10000, 100000] {
        unavailable.push("requires complete fresh1k/10k/100k stage set".into());
    }
    for stage in stages {
        let target = stage["target_records"].as_u64().unwrap_or(0);
        if stage["page_record_limit"] != 32
            || stage["event_payload_bytes"] != 1024
            || stage["ordinary_append_count"] != 16
            || stage["test_expected_vector_bytes"] != 0
        {
            failures.push(format!(
                "stage{target}: inconsistent sample sizes/test buffers"
            ));
        }
        if stage["single_append_max_node_reads"]
            .as_u64()
            .is_none_or(|n| n > 3 * 260)
            || stage["single_append_max_plan_bytes"]
                .as_u64()
                .is_none_or(|n| n > 16 * MIB)
            || stage["max_bulk_plan_bytes"]
                .as_u64()
                .is_none_or(|n| n > 16 * MIB)
        {
            failures.push(format!("stage{target}: operation bounds"));
        }
        let Some(samples) = stage["memory_phases"].as_array() else {
            failures.push(format!("stage{target}: phases absent"));
            continue;
        };
        let names: BTreeSet<_> = samples.iter().filter_map(|s| s["phase"].as_str()).collect();
        if samples.len() != PHASES.len() || names != PHASES.into_iter().collect() {
            failures.push(format!("stage{target}: incomplete/duplicate phases"));
        }
        for s in samples {
            let phase = s["phase"].as_str().unwrap_or("unknown");
            if !measurement_available(s) {
                unavailable.push(format!(
                    "stage{target}/{phase}: process/cache instrumentation unavailable"
                ));
                continue;
            }
            let (Some(m), Some(c)) = (
                s["process"]["private_bytes"].as_u64(),
                s["redb_cache_data_bytes"].as_u64(),
            ) else {
                unavailable.push(format!("stage{target}/{phase}: numeric metrics absent"));
                continue;
            };
            max_memory = Some(max_memory.unwrap_or(0).max(m));
            max_cache = Some(max_cache.unwrap_or(0).max(c));
            if m > 384 * MIB
                || c > 64 * MIB
                || s["configured_cache_bytes"] != 64 * MIB
                || s["test_expected_capacity_bytes"] != 0
            {
                failures.push(format!(
                    "stage{target}/{phase}: absolute/cache/test-buffer bound"
                ));
            }
            if s["debug_assertions"] != true {
                failures.push(format!(
                    "stage{target}/{phase}: requires verification setup"
                ));
            }
            if let Some(pid) = s["same_process"].as_u64() {
                processes.insert(pid);
            } else {
                failures.push(format!("stage{target}/{phase}: process identity absent"));
            }
        }
    }
    if processes.len() > 1 {
        failures.push("samples are from different processes".into());
    }
    let first = stages.iter().find(|s| s["target_records"] == 1000);
    let last = stages.iter().find(|s| s["target_records"] == 100000);
    let mut comparisons = Vec::new();
    let mut page_ratio = None;
    let mut append_ratio = None;
    if let (Some(first), Some(last)) = (first, last) {
        for phase in [&PHASES[4], &PHASES[5]] {
            if let (Some((m1, c1, n1)), Some((mn, cn, nn))) =
                (point(first, phase), point(last, phase))
            {
                let raw = mn - m1;
                let residual = (mn - cn) - (m1 - c1);
                let raw_pass = raw <= i128::from(96 * MIB);
                let residual_pass = residual <= i128::from(32 * MIB);
                let offset = u64::from(*phase == PHASES[5]) * 16;
                if n1 != 1000 + offset || nn != 100000 + offset {
                    failures.push(format!("{phase}: mismatched phase counts"));
                }
                if !raw_pass || !residual_pass {
                    failures.push(format!("{phase}: warm growth bound"));
                }
                comparisons.push(json!({"phase":phase,"baseline_records":n1,"final_records":nn,
                    "baseline_private_bytes":m1,"baseline_cache_bytes":c1,"final_private_bytes":mn,"final_cache_bytes":cn,
                    "raw_growth_bytes":raw,"residual_growth_bytes":residual,"raw_limit_bytes":96*MIB,
                    "residual_limit_bytes":32*MIB,"raw_pass":raw_pass,"residual_pass":residual_pass}));
            } else {
                unavailable.push(format!("{phase}: matched memory points unavailable"));
            }
        }
        for (field, ratio) in [
            ("page32_p95_ms", &mut page_ratio),
            ("append_p95_ms", &mut append_ratio),
        ] {
            if let (Some(a), Some(b)) = (first[field].as_f64(), last[field].as_f64())
                && a.is_finite()
                && b.is_finite()
                && a > 0.0
                && b >= 0.0
            {
                let r = b / a;
                *ratio = Some(r);
                if r > 4.0 {
                    failures.push(format!("{field}: p95 ratio exceeds4"));
                }
            } else {
                failures.push(format!("{field}: latency unavailable"));
            }
        }
    }
    let time_pass = elapsed.is_finite() && elapsed <= 1200.0;
    if !time_pass {
        failures.push("initial fill+read+append exceeds1200s".into());
    }
    json!({"status":if !failures.is_empty(){"FAIL"}else if !unavailable.is_empty(){"UNAVAILABLE"}else{"PASS"},
        "amendment":"long-lived-dialog-a3-memory-budget-amendment-1","phase_comparisons":comparisons,
        "page_p95_ratio":page_ratio,"append_p95_ratio":append_ratio,"ratio_limit":4,
        "max_measured_private_bytes":max_memory,"max_measured_cache_data_bytes":max_cache,
        "process_ids":processes,"elapsed_seconds":elapsed,"time_limit_seconds":1200,"time_pass":time_pass,
        "failures":failures,"unavailable":unavailable})
}

#[test]
fn indexed_scale_gate_signed_bounds_both_phases_and_unavailability() {
    // Arithmetic fixtures only; these are not storage/performance evidence.
    let make = |n: u64, m: u64, c: u64| {
        json!({"target_records":n,"page_record_limit":32,"event_payload_bytes":1024,
        "ordinary_append_count":16,"test_expected_vector_bytes":0,"single_append_max_node_reads":20,
        "single_append_max_plan_bytes":20000,"max_bulk_plan_bytes":1000000,
        "page32_p95_ms":2.0,"append_p95_ms":2.0,
        "memory_phases":PHASES.iter().map(|phase|json!({"phase":phase,
            "records":n+if *phase==PHASES[5]{16}else{0},"process":{"private_bytes":m},
            "redb_cache_data_bytes":c,"configured_cache_bytes":64*MIB,"same_process":7,
            "test_expected_capacity_bytes":0,"debug_assertions":true,
            "process_memory_available":true,"cache_metrics_available":true})).collect::<Vec<_>>()})
    };
    let stages = vec![
        make(1000, 8 * MIB, 2 * MIB),
        make(10000, 32 * MIB, 24 * MIB),
        make(100000, 102 * MIB, 64 * MIB),
    ];
    let good = evaluate(&stages, 1200.0);
    assert_eq!(good["status"], "PASS");
    assert_eq!(
        good["phase_comparisons"][0]["residual_growth_bytes"],
        32 * MIB
    );
    for phase in [4, 5] {
        let mut bad = stages.clone();
        bad[2]["memory_phases"][phase]["process"]["private_bytes"] = json!(102 * MIB + 1);
        assert_eq!(evaluate(&bad, 10.0)["status"], "FAIL");
    }
    let mut raw = stages.clone();
    raw[2]["memory_phases"][4]["process"]["private_bytes"] = json!(104 * MIB + 1);
    assert_eq!(
        evaluate(&raw, 10.0)["phase_comparisons"][0]["raw_pass"],
        false
    );
    let mut negative = stages.clone();
    negative[2] = make(100000, 5 * MIB, MIB);
    let signed = evaluate(&negative, 10.0);
    assert_eq!(signed["status"], "PASS");
    assert_eq!(
        signed["phase_comparisons"][0]["raw_growth_bytes"],
        -3i64 * MIB as i64
    );
    for kind in ["cache", "platform"] {
        let mut absent = stages.clone();
        if kind == "cache" {
            absent[2]["memory_phases"][4]["cache_metrics_available"] = json!(false);
            absent[2]["memory_phases"][4]["redb_cache_data_bytes"] = Value::Null;
        } else {
            absent[2]["memory_phases"][4]["process_memory_available"] = json!(false);
            absent[2]["memory_phases"][4]["process"] = Value::Null;
        }
        assert_eq!(evaluate(&absent, 10.0)["status"], "UNAVAILABLE");
    }
    let mut pid = stages.clone();
    pid[2]["memory_phases"][5]["same_process"] = json!(8);
    assert_eq!(evaluate(&pid, 10.0)["status"], "FAIL");
    let mut cache = stages.clone();
    cache[2]["memory_phases"][5]["redb_cache_data_bytes"] = json!(65 * MIB);
    assert_eq!(evaluate(&cache, 10.0)["status"], "FAIL");
    assert_eq!(evaluate(&stages, 1200.01)["status"], "FAIL");
}
