//! Pure association negatives, not a synthetic three-node/Native origin claim.
use super::*;
use oracle::{
    Directed, History, Item, Request, exact, mappings, pin, progress, proof_joins, request_bytes,
    request_id, request_stable, resources, sent, stopped_owners, stored_request,
};

pub(super) fn contexts() -> [String; 2] {
    ["11".repeat(32), "22".repeat(32)]
}
fn row_fixture(c: &str, slot: u8, role: u8) -> Value {
    json!({"context":c,"slot":slot,"role":role,"enabled":true,"active":true,"lease_current":true,"lease_family":2,"publication_ready":true,"existing_origin_ready":true,"policy_error":null,"handoff_policy_error":null,"publication_error":null,"origin_publication_error":null,"mode":"Active","d":"33".repeat(32),"e":"44".repeat(32),"sid":"A".repeat(43),"q":"55".repeat(32),"cuts":[1,2],"controls":{"committed":"66".repeat(32)},"native_hash":"77".repeat(32),"native_generation":3,"next":if c==contexts()[0]{[1,2]}else{[2,1]},"counts":{"queued":0,"pending":0,"debt":0},"current_consent":"66".repeat(32),"current_consent2":"66".repeat(32),"policy_generation":4,"repair_free":64,"head_pending":null})
}
pub(super) fn status_fixture() -> [Value; 3] {
    let [a, b] = contexts();
    let rows = [
        json!([row_fixture(&a, 0, 0)]),
        json!([row_fixture(&a, 1, 1), row_fixture(&b, 0, 0)]),
        json!([row_fixture(&b, 0, 1)]),
    ];
    std::array::from_fn(
        |i| json!({"contexts":rows[i],"window":[if i==1{[64,64]}else{[128,0]},[0,0],[0,0]],"resources":{"connections":1,"peak_connections":2,"connection_limit":2,"mailbox":1,"peak_mailbox":3,"mailbox_limit":16},"stats":{"committed_encrypt":0,"committed_decrypt":0,"materialized":0,"readable":0,"account_constructors":0,"outbound_constructors":0,"inbound_constructors":0,"aborted_packet_calls":0,"ambiguous_packet_calls":0}}),
    )
}
pub(super) fn directed(role: u8, previous: Option<old::Id>) -> Directed {
    Directed {
        context: contexts()[1].clone(),
        author: if role == 0 { 1 } else { 2 },
        role,
        id: 500 + u64::from(role),
        body: "G3 exact test text".into(),
        ordinal: if role == 0 { 2 } else { 1 },
        sequence: 71 + u64::from(role),
        previous,
        saved: Request {
            queue: 19,
            event: Some("88".repeat(32)),
            delivered: true,
        },
    }
}
fn core(d: &Directed) -> Result<wire::Core2> {
    Ok(wire::Core2 {
        c: state::parse_id(&d.context)?,
        d: [3; 32],
        e: [4; 32],
        sid: "A".repeat(43),
        role: d.role,
        n: d.ordinal,
        previous: d.previous,
        sequence: d.sequence,
        kind: 3,
        request: request_bytes(d.id),
        parents: d.previous.into_iter().collect(),
        expected: kilogram_ratchet::ordered_v2::Expected {
            g: 0,
            index: 0,
            parent: None,
            tail: None,
        },
        author_auth: [5; 32],
        q: [6; 32],
    })
}
pub(super) fn item(d: &Directed) -> Item {
    Item {
        format: 2,
        id: d.saved.event.clone().unwrap_or_default(),
        role: d.role,
        ordinal: d.ordinal,
        sequence: d.sequence,
        request: request_id(d.id),
        text: d.body.clone(),
        proof_kind: "FullMixed".into(),
        delivered: true,
    }
}
#[test]
fn g3_mapping_and_actual_owner_guard_reject_view_shortcuts() -> Result<()> {
    let s = status_fixture();
    let m = mappings(&s, &contexts())?;
    assert_eq!(mapping(&m, 1, &contexts()[0])?.slot, 1);
    assert_eq!(mapping(&m, 1, &contexts()[1])?.slot, 0);
    let ready = s
        .iter()
        .map(|status| -> Result<Value> {
            let mut rows = status["contexts"]
                .as_array()
                .context("fixture contexts")?
                .clone();
            rows.sort_by_key(|row| row["slot"].as_u64());
            Ok(json!({"storage_stage":"Ready","zero_migration_native_calls":true,"contexts":rows}))
        })
        .collect::<Result<Vec<_>>>()?;
    oracle::ready_mappings(&ready, &m)?;
    let mut wrong_ready = ready.clone();
    wrong_ready[1]["contexts"]
        .as_array_mut()
        .context("Ready rows")?
        .swap(0, 1);
    assert!(oracle::ready_mappings(&wrong_ready, &m).is_err());
    let mut wrong_ready = ready.clone();
    wrong_ready[2]["contexts"][0]["role"] = json!(0);
    assert!(oracle::ready_mappings(&wrong_ready, &m).is_err());
    assert!(oracle::ready_mappings(&ready[..2], &m).is_err());
    Owners::default().stopped()?;
    let mut missing = Owners {
        required: [false, true, true],
        ..Default::default()
    };
    assert!(missing.alive().is_err());
    assert!(Owners::default().require([false, true, true]).is_err());
    oracle::owner_presence([false, true, true], [false, true, true])?;
    assert!(oracle::owner_presence([true, true, true], [false, true, true]).is_err());
    assert!(oracle::owner_presence([false, true, false], [false, true, true]).is_err());
    assert!(
        Live {
            status: [None, None, None],
            history: Default::default()
        }
        .status(0)
        .is_err()
    );
    for owner in 0..3 {
        let mut actual = [false; 3];
        actual[owner] = true;
        assert!(stopped_owners(actual).is_err());
    }
    for (field, value) in [
        ("slot", json!(0)),
        ("role", json!(2)),
        ("context", json!(contexts()[1])),
    ] {
        let mut bad = s.clone();
        bad[1]["contexts"][0][field] = value;
        assert!(mappings(&bad, &contexts()).is_err());
    }
    let mut duplicate = s.clone();
    duplicate[0]["contexts"] = json!([s[0]["contexts"][0], s[0]["contexts"][0]]);
    assert!(mappings(&duplicate, &contexts()).is_err());
    Ok(())
}
#[test]
fn g3_directed_history_and_core_bind_both_author_lanes() -> Result<()> {
    for d in [directed(0, Some([9; 32])), directed(1, None)] {
        let mut original = item(&d);
        original.id = "aa".repeat(32);
        original.format = 1;
        original.role = 0;
        original.ordinal = 1;
        original.sequence = 1;
        original.request = "ec000000000000000000000000000002".into();
        original.text = "genuine C source 2".into();
        original.proof_kind = "LegacySnapshot".into();
        let before = History::from([(original.id.clone(), original.clone())]);
        let good = item(&d);
        let mut after = before.clone();
        after.insert(good.id.clone(), good.clone());
        d.history(&before, &after)?;
        let c = core(&d)?;
        d.core(&c)?;
        for changed in [
            "role",
            "ordinal",
            "sequence",
            "request",
            "text",
            "proof_kind",
            "delivered",
        ] {
            let mut bad = serde_json::to_value(&good)?;
            bad[changed] = match changed {
                "role" => json!(1 - d.role),
                "ordinal" => json!(9),
                "sequence" => json!(1),
                "delivered" => json!(false),
                _ => json!("wrong"),
            };
            let bad: Item = serde_json::from_value(bad)?;
            let mut wrong = before.clone();
            wrong.insert(bad.id.clone(), bad);
            assert!(d.history(&before, &wrong).is_err());
        }
        let mut bad = c.clone();
        bad.previous = Some([1; 32]);
        assert!(d.core(&bad).is_err());
        let mut bad = c.clone();
        bad.c = [1; 32];
        assert!(d.core(&bad).is_err());
        let mut bad = c.clone();
        bad.sequence = 0;
        assert!(d.core(&bad).is_err());
        let mut extra = after.clone();
        let mut r = good.clone();
        r.id = "99".repeat(32);
        extra.insert(r.id.clone(), r);
        assert!(d.history(&before, &extra).is_err());
        let mut changed = after;
        changed.get_mut(&original.id).context("row")?.text = "earlier changed".into();
        assert!(d.history(&before, &changed).is_err());
    }
    Ok(())
}
#[test]
fn g3_request_and_actual_proof_association_refuse_wrong_captures() -> Result<()> {
    let d = directed(0, Some([9; 32]));
    let a = sent("[[19],[]]")?;
    request_stable(&a, &d.saved)?;
    for bad in [
        json!([[[19], [], 1]]),
        json!([[[], [], 0]]),
        json!([[[19], null, 0]]),
    ] {
        assert!(oracle::request(bad).is_err());
    }
    let mut bad = d.saved.clone();
    bad.queue += 1;
    assert!(request_stable(&a, &bad).is_err());
    let stored = store::L1Request {
        queue_sequence: Some(19),
        event: Some(state::parse_id(d.saved.event.as_deref().context("ID")?)?),
        text_hash: old::hash(b"kg:od1:request-text\0", d.body.as_bytes()),
    };
    stored_request(&stored, &d)?;
    let mut bad = stored.clone();
    bad.queue_sequence = None;
    assert!(stored_request(&bad, &d).is_err());
    let mut bad = stored.clone();
    bad.queue_sequence = Some(20);
    assert!(stored_request(&bad, &d).is_err());
    let mut bad = stored.clone();
    bad.event = Some([1; 32]);
    assert!(stored_request(&bad, &d).is_err());
    let mut bad = stored;
    bad.text_hash = [0; 32];
    assert!(stored_request(&bad, &d).is_err());
    for own in [false, true] {
        let p = store::L1ProofRefs {
            original: [1; 32],
            admission: (!own).then_some([2; 32]),
            final_auth: Some([1; 32]),
            receiver: (!own).then_some([3; 32]),
            receipt: (!own).then_some([4; 32]),
            receipt_overlay: Some([4; 32]),
            receiver_overlay: Some([3; 32]),
        };
        proof_joins(&p, [1; 32], [3; 32], [4; 32], own)?;
        let mut bad = p.clone();
        bad.original = [5; 32];
        assert!(proof_joins(&bad, [1; 32], [3; 32], [4; 32], own).is_err());
        let mut bad = p.clone();
        bad.receipt_overlay = Some([5; 32]);
        assert!(proof_joins(&bad, [1; 32], [3; 32], [4; 32], own).is_err());
        let mut bad = p.clone();
        bad.receiver_overlay = None;
        assert!(proof_joins(&bad, [1; 32], [3; 32], [4; 32], own).is_err());
        let mut bad = p;
        bad.final_auth = None;
        assert!(proof_joins(&bad, [1; 32], [3; 32], [4; 32], own).is_err());
    }
    Ok(())
}
#[test]
fn g3_pin_isolation_preserves_context_facts_not_global_counters() -> Result<()> {
    let s = status_fixture();
    let before = s[1].clone();
    let mut after = before.clone();
    after["contexts"][0]["lease_current"] = json!(false);
    after["contexts"][0]["policy_error"] = json!("revoked Y");
    after["contexts"][0]["handoff_policy_error"] = json!("revoked Y");
    after["contexts"][0]["policy_generation"] = json!(5);
    after["root_generation"] = json!(99);
    after["shared_authority_used"] = json!(512);
    pin(&before, &after, &contexts())?;
    for (owner_context, key, value) in [
        (0, "native_hash", json!("99".repeat(32))),
        (0, "q", json!("99".repeat(32))),
        (1, "policy_generation", json!(5)),
        (1, "current_consent2", json!("99".repeat(32))),
        (0, "current_consent", json!("99".repeat(32))),
        (1, "repair_free", json!(63)),
        (0, "lease_current", json!(true)),
        (0, "policy_generation", json!(4)),
    ] {
        let mut bad = after.clone();
        bad["contexts"][owner_context][key] = value;
        assert!(pin(&before, &bad, &contexts()).is_err());
    }
    Ok(())
}
#[test]
fn g3_owner_resource_limits_refuse_borrowing_or_overruns() -> Result<()> {
    let s = status_fixture();
    resources(&s[0], &[0])?;
    resources(&s[1], &[1, 0])?;
    // Resource fields are independently observed; both must stay within caps.
    let mut noncoherent = s[1].clone();
    noncoherent["resources"]["mailbox"] = json!(4);
    noncoherent["resources"]["peak_mailbox"] = json!(3);
    resources(&noncoherent, &[0, 1])?;
    let mut held = s[1].clone();
    held["window"] = json!([[64, 64], [64, 64], [64, 64]]);
    resources(&held, &[1, 0])?;
    held["window"][1][1] = json!(0);
    resources(&held, &[1, 0])?;
    held["window"][1][0] = json!(65);
    held["window"][2][0] = json!(65);
    assert!(resources(&held, &[1, 0]).is_err());
    for (field, value) in [
        ("peak_connections", 3),
        ("connection_limit", 3),
        ("peak_mailbox", 17),
        ("mailbox", 17),
    ] {
        let mut bad = s[1].clone();
        bad["resources"][field] = json!(value);
        assert!(resources(&bad, &[0, 1]).is_err());
    }
    let mut bad = s[1].clone();
    bad["window"][0] = json!([128, 0]);
    assert!(resources(&bad, &[0, 1]).is_err());
    Ok(())
}
#[test]
fn g3_directed_progress_and_diagnostic_paths_reject_hidden_work() -> Result<()> {
    let s = status_fixture();
    let maps = mappings(&s, &contexts())?;
    let d = directed(0, None);
    let mut after = s.clone();
    for owner in [1, 2] {
        let i = if owner == 1 { 1 } else { 0 };
        after[owner]["contexts"][i]["next"] = json!([3, 1]);
        after[owner]["contexts"][i]["native_generation"] = json!(4);
        for field in if owner == 1 {
            ["committed_encrypt", "materialized"]
        } else {
            ["committed_decrypt", "readable"]
        } {
            after[owner]["stats"][field] = json!(1);
        }
    }
    progress(&s.clone().map(Some), &after.clone().map(Some), &maps, &d)?;
    for consent in ["current_consent", "current_consent2"] {
        let mut bad = after.clone();
        bad[1]["contexts"][1][consent] = json!("99".repeat(32));
        assert!(progress(&s.clone().map(Some), &bad.map(Some), &maps, &d).is_err());
    }
    let mut subset = after.clone().map(Some);
    subset[2] = None;
    assert!(progress(&s.clone().map(Some), &subset, &maps, &d).is_err());
    let mut bad = after.clone();
    bad[2]["stats"]["committed_decrypt"] = json!(0);
    assert!(progress(&s.clone().map(Some), &bad.map(Some), &maps, &d).is_err());
    let mut bad = after;
    bad[1]["contexts"][1]["next"] = json!([4, 1]);
    assert!(progress(&s.clone().map(Some), &bad.map(Some), &maps, &d).is_err());
    let error = exact(
        "owner1/C1",
        &json!({"native_hash":"a".repeat(10000)}),
        &json!({"native_hash":"b".repeat(10000)}),
    )
    .err()
    .context("different Native")?
    .to_string();
    assert!(error.contains("owner1/C1/native_hash") && error.len() < 1024);
    Ok(())
}
#[test]
fn g3_protocol_capture_parsers_do_not_grant_structural_bytes_authority() -> Result<()> {
    let raw = old::encode(
        &old::a([old::u(1), old::b([0; 64]), old::b([0; 64])]),
        wire::MAX_AUTH,
    )?;
    let parsed = wire::ParsedCredentialBundle::parse(&raw)?;
    assert!(parsed.verify_root().is_err());
    assert!(wire::ParsedEvent2::parse(&raw).is_err());
    assert!(wire::ParsedReceipt2::parse(&raw).is_err());
    Ok(())
}

#[test]
fn g3_stored_own_request_distinguishes_imported_legacy_and_current_queue() -> Result<()> {
    let event = [44; 32];
    let text = "original migrated text";
    let legacy = store::L1Request {
        queue_sequence: None,
        event: Some(event),
        text_hash: old::hash(b"kg:od1:request-text\0", text.as_bytes()),
    };
    let current = store::L1Request {
        queue_sequence: Some(19),
        ..legacy.clone()
    };
    oracle::stored_own_request(&legacy, event, text, 1)?;
    oracle::stored_own_request(&current, event, text, 2)?;
    assert!(oracle::stored_own_request(&current, event, text, 1).is_err());
    assert!(oracle::stored_own_request(&legacy, event, text, 2).is_err());
    for format in [0, 3] {
        assert!(oracle::stored_own_request(&legacy, event, text, format).is_err());
        assert!(oracle::stored_own_request(&current, event, text, format).is_err());
    }
    for (format, valid) in [(1, legacy), (2, current)] {
        let mut bad = valid.clone();
        bad.event = None;
        assert!(oracle::stored_own_request(&bad, event, text, format).is_err());
        bad.event = Some([45; 32]);
        assert!(oracle::stored_own_request(&bad, event, text, format).is_err());
        let mut bad = valid;
        bad.text_hash = [0; 32];
        assert!(oracle::stored_own_request(&bad, event, text, format).is_err());
    }
    Ok(())
}
