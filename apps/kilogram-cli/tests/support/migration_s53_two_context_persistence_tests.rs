//! Public association models only; real encoding/Work charging is checked in state.
use super::*;

fn c2() -> String {
    state::hex(&[2; 32])
}
fn c1() -> String {
    state::hex(&[1; 32])
}
fn locator(revision: u64) -> Result<Vec<u8>> {
    Ok(wire::ParsedRuntimeLocator2::sign(
        wire::RuntimeLocatorFields2 {
            c: [2; 32],
            cert: [3; 32],
            peer_cert: [4; 32],
            endpoint: [5; 32],
            ips: vec![],
            relay: Some("http://127.0.0.1:40123/".into()),
            revision,
            not_before: 100,
            expires: 200,
        },
        &kilogram_identity::DeviceIdentity::from_secret_bytes([9; 32]),
    )?
    .raw()
    .to_vec())
}
fn row(owner: usize, c: &str, slot: u8, role: u8, texts: [u8; 2]) -> Result<Value> {
    let mut history = oracle::History::new();
    let mut tails = [None, None];
    let mut sources = Vec::new();
    for lane in 0..2 {
        let mut last = [20 + lane as u8; 32];
        for n in 1..=texts[lane] {
            last = [40 + lane as u8 * 10 + n; 32];
            let id = state::hex(&last);
            history.insert(
                id.clone(),
                oracle::Item {
                    format: 2,
                    id,
                    role: lane as u8,
                    ordinal: u64::from(n),
                    sequence: 70 + u64::from(n),
                    request: oracle::request_id(500 + u64::from(n)),
                    text: format!("public text {lane}/{n}"),
                    proof_kind: "FullMixed".into(),
                    delivered: true,
                },
            );
        }
        tails[lane] = Some(last);
        sources.push(json!({"role":lane,"next":texts[lane]+1,"ordinal":texts[lane],"kind":if texts[lane]==0{1}else{3},"last":state::hex(&last)}));
    }
    let meta = store::L1Metadata {
        policy_generation: 4,
        wall_high_water: 127,
        next: texts.map(|n| u64::from(n) + 1),
        tails,
        next_queue: 4,
        request_index_after: None,
        request_index_ready: true,
        ack_next: u64::from(texts[1 - role as usize]) + 1,
        peer_source_next: u64::from(texts[1 - role as usize]) + 1,
        peer_source_last: tails[1 - role as usize],
    };
    let existing = store::ExistingState7 {
        mode: store::ExistingMode::Active,
        current_consent: Some([7; 32]),
        selected_consent: Some([7; 32]),
        q: Some([8; 32]),
        locator_revision: [127, 127],
        ..Default::default()
    };
    let routing = routes::Stored {
        own: locator(127)?,
        peer: locator(127)?,
    };
    let fixed = store::L1FixedRecords7 {
        head: fingerprint(&[11; 100]),
        legacy_budget: fingerprint(&[12; 80]),
        own_locator: Some(fingerprint(&routing.own)),
        peer_locator: Some(fingerprint(&routing.peer)),
        existing: Some(fingerprint(&packed(&existing)?)),
        metadata: Some(fingerprint(&packed(&meta)?)),
    };
    Ok(
        json!({"mapping":oracle::Mapping{owner,context:c.into(),slot,role},"identity":{"account":owner},"routing":routing,"existing":existing,"metadata":meta,"fixed":fixed,"policy_generation":4,"history_initialized":true,"sources":sources,"repair":{"repair_free":64,"outstanding":0,"pending":0,"head":null,"grant_present":false},"stable":{"history":history,"native":"opaque hash","head":"inner Head hash"}}),
    )
}
fn reports(texts: [u8; 2]) -> Result<([Value; 3], Vec<oracle::Mapping>)> {
    let a = c1();
    let b = c2();
    let maps = vec![
        oracle::Mapping {
            owner: 0,
            context: a.clone(),
            slot: 0,
            role: 0,
        },
        oracle::Mapping {
            owner: 1,
            context: a.clone(),
            slot: 1,
            role: 1,
        },
        oracle::Mapping {
            owner: 1,
            context: b.clone(),
            slot: 0,
            role: 0,
        },
        oracle::Mapping {
            owner: 2,
            context: b.clone(),
            slot: 0,
            role: 1,
        },
    ];
    let mut result = [Value::Null, Value::Null, Value::Null];
    for (owner, value) in result.iter_mut().enumerate() {
        let cfg = db::migration7::MigrationConfig {
            logical_quota: [1 << 30; 2],
            physical_capacity: 4 << 30,
            output_profile: PathBuf::from("E:/test/indexed.profile"),
            control_ipc_file: PathBuf::from("E:/test/indexed.ipc"),
            locator_directory: PathBuf::from("E:/test/locators"),
        };
        let accounting = store::L1Accounting {
            runtime: store::L1AccountingGroup {
                records: 40,
                used: [100000; 2],
                reserved: [200000; 2],
            },
            prepaid_metadata: store::L1AccountingGroup {
                records: 5,
                used: [400, 0],
                reserved: [10000, 0],
            },
        };
        *value = json!({"owner":owner,"contexts":{},"authority":{"saved":"public snapshot hash"},"next_author":81,"initialized":true,"config":cfg,"profile":{"config_revision":7,"logical_quota":cfg.logical_quota,"physical_capacity":cfg.physical_capacity,"route":{"policy":"relay-only","relay":"http://127.0.0.1:40123/","bind":null},"enabled_contexts":[a,b]},"capacity":{"revision":7,"hash":"33".repeat(32)},"backend":{"physical_quota":cfg.physical_capacity},"credits":{"used":[100400,100000],"reserved":[210000,200000],"repair_free":[64,64],"grant_unissued":[0,0],"pending":[0,0],"head_in_use":[false,false]},"accounting":accounting,"generation":91});
        for m in maps.iter().filter(|m| m.owner == owner) {
            value["contexts"][&m.context] = row(owner, &m.context, m.slot, m.role, texts)?;
        }
    }
    Ok((result, maps))
}
fn routed(before: &Value) -> Result<Value> {
    let status = json!({"root_generation":92,"contexts":[{"context":c2(),"own_locator_revision":128,"publication_ready":true,"wall_high_water":128}]});
    solo_bridge(before, &status, &c2(), &locator(128)?)
}
#[test]
fn g3_routing_whitelist_actual_lengths_reject_excluded_drift_count_slot_and_overflow() -> Result<()>
{
    let (base, _) = reports([3, 1])?;
    let before = &base[1];
    let good = routed(before)?;
    routing(before, &good, &c2(), true)?;
    for key in ["head", "legacy_budget"] {
        let mut bad = good.clone();
        bad["contexts"][c2()]["fixed"][key]["digest"] = json!(vec![0; 32]);
        assert!(routing(before, &bad, &c2(), true).is_err());
    }
    for key in ["ack_next", "policy_generation", "next_queue"] {
        let mut bad = good.clone();
        bad["contexts"][c2()]["metadata"][key] = json!(99);
        let m: store::L1Metadata = typed(context(&bad, &c2())?, "metadata")?;
        bad["contexts"][c2()]["fixed"]["metadata"] = json!(fingerprint(&packed(&m)?));
        assert!(routing(before, &bad, &c2(), true).is_err());
    }
    let mut bad = good.clone();
    bad["contexts"][c2()]["existing"]["control_debt"] = json!(true);
    let e: store::ExistingState7 = typed(context(&bad, &c2())?, "existing")?;
    bad["contexts"][c2()]["fixed"]["existing"] = json!(fingerprint(&packed(&e)?));
    assert!(routing(before, &bad, &c2(), true).is_err());
    let mut bad = good.clone();
    bad["accounting"]["runtime"]["records"] = json!(41);
    assert!(routing(before, &bad, &c2(), true).is_err());
    let mut bad = good.clone();
    bad["accounting"]["runtime"]["used"] = json!([100000, 100001]);
    assert!(routing(before, &bad, &c2(), true).is_err());
    let mut bad = good.clone();
    bad["contexts"][c2()]["fixed"]["own_locator"] = Value::Null;
    assert!(routing(before, &bad, &c2(), true).is_err());
    let mut bad = good.clone();
    bad["contexts"][c2()]["fixed"]["own_locator"]["length"] = json!(u64::MAX);
    assert!(routing(before, &bad, &c2(), true).is_err());
    let mut bad = good;
    bad["accounting"]["runtime"]["used"][0] = json!(u64::MAX);
    bad["accounting"]["runtime"]["reserved"][0] = json!(1);
    assert!(routing(before, &bad, &c2(), true).is_err());
    Ok(())
}
#[test]
fn g3_capacity_preserves_runtime_after_routing_and_refuses_wrong_paths_quotas_cas_profile()
-> Result<()> {
    let (base, _) = reports([3, 1])?;
    let before = routed(&base[1])?;
    let candidate = candidate(&before)?;
    let mut after = before.clone();
    after["config"] = json!(candidate);
    after["profile"] = desired_profile(&before, &candidate)?;
    after["capacity"] = json!({"revision":8,"hash":"44".repeat(32)});
    after["backend"]["physical_quota"] = json!(candidate.physical_capacity);
    after["accounting"]["prepaid_metadata"]["used"][0] = json!(401);
    after["accounting"]["prepaid_metadata"]["reserved"][0] = json!(9999);
    let used = oracle::number(&json!({"v":after["credits"]["used"][0]}), "v")?;
    after["credits"]["used"][0] = json!(used + 1);
    let reserved = after["credits"]["reserved"][0]
        .as_u64()
        .context("fixture reserved")?;
    after["credits"]["reserved"][0] = json!(reserved - 1);
    capacity(&before, &after, &candidate, &c2())?;
    // A real routing delta is not a Capacity accounting allowance.
    assert!(capacity(&base[1], &after, &candidate, &c2()).is_err());
    for (key, value) in [
        ("config_revision", json!(7)),
        ("route", json!({"policy":"auto"})),
    ] {
        let mut bad = after.clone();
        bad["profile"][key] = value;
        assert!(capacity(&before, &bad, &candidate, &c2()).is_err());
    }
    let mut bad = after.clone();
    bad["capacity"]["revision"] = json!(7);
    assert!(capacity(&before, &bad, &candidate, &c2()).is_err());
    let mut bad = after.clone();
    bad["capacity"]["hash"] = before["capacity"]["hash"].clone();
    assert!(capacity(&before, &bad, &candidate, &c2()).is_err());
    let mut wrong = candidate;
    wrong.output_profile = PathBuf::from("E:/wrong/profile");
    assert!(capacity(&before, &after, &wrong, &c2()).is_err());
    let mut overflow = before;
    overflow["config"]["physical_capacity"] = json!(u64::MAX);
    assert!(super::candidate(&overflow).is_err());
    Ok(())
}
#[test]
fn g3_settled_barrier_uses_control_ordinal_zero_and_refuses_delayed_ack_or_watermark() -> Result<()>
{
    for texts in [[0, 0], [3, 1]] {
        let (reports, maps) = reports(texts)?;
        settled(&reports, &maps, &c2())?;
        for key in [
            "ack_next",
            "peer_source_next",
            "peer_source_last",
            "request_index_ready",
        ] {
            let mut bad = reports.clone();
            bad[1]["contexts"][c2()]["metadata"][key] = match key {
                "peer_source_last" => json!(vec![0; 32]),
                "request_index_ready" => json!(false),
                _ => json!(0),
            };
            assert!(settled(&bad, &maps, &c2()).is_err());
        }
        let mut bad = reports.clone();
        bad[1]["contexts"][c2()]["sources"][1]["ordinal"] = json!(9);
        assert!(settled(&bad, &maps, &c2()).is_err());
        let mut bad = reports.clone();
        bad[1]["initialized"] = json!(false);
        assert!(settled(&bad, &maps, &c2()).is_err());
        let mut bad = reports;
        bad[1]["contexts"][c2()]["stable"]["history"] = json!({});
        if texts != [0, 0] {
            assert!(settled(&bad, &maps, &c2()).is_err());
        }
    }
    Ok(())
}
