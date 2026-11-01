//! Pure metadata failure cases. No enrollment, product child or private-state read.
use super::*;

fn id(n: u64) -> old::Id {
    let mut out = [0; 32];
    out[..8].copy_from_slice(&n.to_be_bytes());
    out
}
fn snapshot() -> Result<Snapshot> {
    let anchor = id(384);
    let mut previous = anchor;
    let mut sources = Vec::new();
    for n in 385..=640 {
        let request = oracle::Request::new(0, 0, n)?;
        let event = id(u64::from(n));
        sources.push(Source {
            ordinal: u64::from(n),
            request: source::parse_request_id(&source::request_id(request))?,
            id: event,
            previous,
            source_hash: id(1000 + u64::from(n)),
            ciphertext_hash: id(2000 + u64::from(n)),
            text_hash: old::hash(b"kg:od1:request-text\0", request.body().as_bytes()),
        });
        previous = event;
    }
    Ok(Snapshot {
        state: PathBuf::from("synthetic-owned-state"),
        scope: Scope {
            source: id(1),
            migration: id(2),
            context: id(3),
            q: id(4),
            d: id(5),
            e: id(6),
            sid: "original-synthetic-sid".into(),
            identity: id(7),
        },
        anchor,
        sources,
        native_content: id(8),
        native_seal: id(9),
        c2_identity: id(10),
    })
}
fn row() -> Value {
    json!({"context":state::hex(&id(3)),"role":0,"mode":"Active","lease_current":false,"lease_generation":null,"policy_error":null,"handoff_policy_error":null,"next":[641,640],"counts":{"queued":0,"pending":0,"debt":256}})
}

#[test]
fn s54_p7_source_chain_rejects_missing_duplicate_reorder_and_wrong_anchor() -> Result<()> {
    let good = snapshot()?;
    good.validate()?;
    for change in 0..4 {
        let mut bad = good.clone();
        match change {
            0 => {
                bad.sources.pop();
            }
            1 => bad.sources[8] = bad.sources[7].clone(),
            2 => bad.sources.swap(7, 8),
            _ => bad.anchor = id(999),
        }
        ensure!(
            bad.validate().is_err(),
            "reject broken source chain {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_request_association_rejects_role_context_body_and_previous_alias() -> Result<()> {
    let good = snapshot()?;
    for change in 0..4 {
        let mut bad = good.clone();
        let source = &mut bad.sources[7];
        match change {
            0 => {
                source.request =
                    source::parse_request_id(&source::request_id(oracle::Request::new(0, 1, 392)?))?
            }
            1 => {
                source.request =
                    source::parse_request_id(&source::request_id(oracle::Request::new(1, 0, 8)?))?
            }
            2 => source.text_hash = old::hash(b"kg:od1:request-text\0", b"different plaintext"),
            _ => source.previous = id(385),
        }
        ensure!(
            bad.validate().is_err(),
            "reject original request association {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_reopen_rejects_changed_source_ciphertext_and_event_association() -> Result<()> {
    let before = snapshot()?;
    before.same(&before)?;
    for change in 0..3 {
        let mut after = before.clone();
        match change {
            0 => after.sources[7].source_hash = id(999),
            1 => after.sources[7].ciphertext_hash = id(999),
            _ => {
                after.sources[7].id = id(999);
                after.sources[8].previous = id(999);
            }
        }
        after.validate()?;
        ensure!(
            before.same(&after).is_err(),
            "valid chain alone cannot hide immutable-source change {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_reopen_rejects_state_scope_original_identity_and_c2_substitution() -> Result<()> {
    let before = snapshot()?;
    for change in 0..11 {
        let mut after = before.clone();
        match change {
            0 => after.state = PathBuf::from("different-state"),
            1 => after.scope.source = id(999),
            2 => after.scope.migration = id(999),
            3 => after.scope.context = id(999),
            4 => after.scope.q = id(999),
            5 => after.scope.d = id(999),
            6 => after.scope.e = id(999),
            7 => after.scope.sid = "different-sid".into(),
            8 => after.scope.identity = id(999),
            9 => after.c2_identity = id(999),
            _ => after.anchor = id(999),
        }
        ensure!(
            before.same(&after).is_err(),
            "reject scope substitution {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_native_comparison_accepts_outer_rewrap_but_rejects_changed_content() -> Result<()> {
    let before = snapshot()?;
    let mut after = before.clone();
    after.native_seal = id(999);
    before.same(&after)?;
    after.native_content = id(999);
    ensure!(
        before.same(&after).is_err(),
        "opened content change requires investigation, not inferred new session"
    );
    Ok(())
}
#[test]
fn s54_p7_queued_or_pending_acceptance_is_not_materialized_offline_debt() -> Result<()> {
    ensure!(offline_row(&row(), 641, 256)?, "materialized frontier");
    for change in 0..4 {
        let mut actual = row();
        match change {
            0 => {
                actual["next"][0] = 385.into();
                actual["counts"]["queued"] = 256.into();
                actual["counts"]["debt"] = 0.into();
            }
            1 => actual["counts"]["pending"] = 1.into(),
            2 => actual["counts"]["queued"] = 1.into(),
            _ => actual["counts"]["debt"] = 255.into(),
        }
        ensure!(
            !offline_row(&actual, 641, 256)?,
            "do not call acceptance materialization {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_offline_rejects_live_lease_expired_permission_or_readable_b640() -> Result<()> {
    for change in 0..7 {
        let mut actual = row();
        match change {
            0 => actual["lease_current"] = true.into(),
            1 => actual["lease_generation"] = 12.into(),
            2 => actual["policy_error"] = "expired".into(),
            3 => actual["handoff_policy_error"] = "revoked".into(),
            4 => actual["next"][1] = 641.into(),
            5 => actual["role"] = 1.into(),
            _ => actual["mode"] = "Selected".into(),
        }
        ensure!(
            offline_row(&actual, 641, 256).is_err(),
            "reject invalid offline premise {change}"
        );
    }
    Ok(())
}
#[test]
fn s54_p7_reopen_rejects_each_native_attempt_effect_or_constructor() -> Result<()> {
    no_native_calls(oracle::Counts::default())?;
    for key in [
        "encrypt",
        "decrypt",
        "attempts",
        "aborted",
        "ambiguous",
        "accounts",
        "outbound",
        "inbound",
    ] {
        let mut value = json!({"encrypt":0,"decrypt":0,"attempts":0,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0});
        value[key] = 1.into();
        ensure!(
            no_native_calls(oracle::Counts::read(&value)?).is_err(),
            "actual new-incarnation {key} cannot be inferred zero"
        );
    }
    Ok(())
}
