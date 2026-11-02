//! Supplied full geometry; signed bridges are small and explicitly separate.
use super::*;
use fixture::Fixture;
#[test]
fn s54_b1b_complete_directed_union_consumes_exact_current_capacities() -> Result<()> {
    let f = Fixture::new()?;
    let a = f.run()?;
    assert!(
        a.totals.current.encrypt == 1304
            && a.totals.current.decrypt == 1305
            && a.totals.current.attempts == 2609
            && a.totals.old
                == Work {
                    encrypt: 52,
                    decrypt: 52
                }
            && a.totals.overlap
                == Work {
                    encrypt: 8,
                    decrypt: 9
                }
            && a.totals.union
                == Work {
                    encrypt: 1348,
                    decrypt: 1348
                }
            && a.totals.directed == 2696
            && (a.totals.accounts, a.totals.outbound, a.totals.inbound) == (4, 2, 2)
    );
    let mut forged = f.model.ops.summary()?;
    forged.counts.encrypt += 1;
    assert!(Draft::new(&f.early, &f.model.ledger, &forged, &f.model.outcomes).is_err());
    assert!(
        a.ledger.closed.iter().any(|c| c.tail.is_some())
            && a.ledger
                .closed
                .iter()
                .filter(|c| c.tail.is_some())
                .all(|c| c.prefix.entries.iter().all(|p| p.operation != c.operation))
    );
    Ok(())
}
#[test]
fn s54_b1b_overlap_omitted_duplicate_foreign_or_swapped_despite_equal_totals_refuses() -> Result<()>
{
    let mut f = Fixture::new()?;
    let original = f.early.clone();
    let p = *original.overlap.keys().next().context("overlap")?;
    for change in 0..8 {
        f.early = original.clone();
        let bound = f.early.overlap.get_mut(&p).context("bound")?;
        match change {
            0 => {
                f.early.overlap.remove(&p);
            }
            1 => bound.direction = 1 - bound.direction,
            2 => bound.slot = 1 - bound.slot,
            3 => bound.mark.key.nonce = [99; 32],
            4 => bound.mark.start = 4,
            5 => bound.mark.prefix_sequence += 1,
            6 => {
                let b = f.early.overlap.remove(&p).context("b")?;
                f.early.overlap.insert(Position { owner: 1, ..p }, b);
            }
            _ => {
                let b = f.early.overlap.remove(&p).context("b")?;
                f.early.overlap.insert(Position { n: 15, ..p }, b);
            }
        }
        assert!(f.run().is_err(), "overlap mutation {change}");
    }
    f.early = original;
    let fact = f.early.old.get(&p).context("fact")?.fact.clone();
    let b = f.early.overlap[&p].clone();
    assert!(f.early.bind_late(&f.model.ledger, &[fact], b.mark).is_err());
    Ok(())
}
#[test]
fn s54_b1b_original_raw_cipher_projection_and_event_binding_cannot_be_replaced() -> Result<()> {
    let mut f = Fixture::new()?;
    let original = f.early.clone();
    let p = *original
        .overlap
        .keys()
        .find(|p| p.author == 1)
        .context("late")?;
    for field in 0..8 {
        f.early = original.clone();
        let e = f.early.old.get_mut(&p).context("old")?;
        match field {
            0 => e.fact.atom.raw = [91; 32],
            1 => e.fact.atom.cipher = [92; 32],
            2 => e.fact.atom.body = Some([93; 32]),
            3 => e.fact.atom.event = [94; 32],
            4 => e.fact.atom.author = 0,
            5 => e.fact.atom.context = [95; 32],
            6 => e.fact.source = [96; 32],
            _ => e.migration = true,
        };
        assert!(f.run().is_err(), "source substitution {field}");
    }
    Ok(())
}
#[test]
fn s54_b1b_b25_old104_chronological106_and_c2_event1_stay_distinct() -> Result<()> {
    let mut f = Fixture::new()?;
    let original = f.early.clone();
    for change in 0..5 {
        f.early = original.clone();
        match change {
            0 => {
                f.early.b25.remove(&0);
            }
            1 => f.early.b25.get_mut(&1).context("B25")?.event = [98; 32],
            2 => f.early.b25.get_mut(&0).context("B25")?.body = [99; 32],
            3 => {
                f.early.stages[2] = Some(Work {
                    encrypt: 53,
                    decrypt: 53,
                })
            }
            _ => f.early.b25.get_mut(&0).context("B25")?.bound.direction = 0,
        };
        assert!(f.run().is_err(), "B25 {change}");
    }
    f.early = original;
    let row = f.rows.iter().find(|r| r.context == [4; 32]).context("C2")?;
    assert_eq!(row.format, 1);
    assert!(
        f.early
            .insert(Fact::new(0, [1; 32], [1; 32], Atom::record(1, row))?)
            .is_err()
    );
    let duplicate = f.early.b25[&0].clone();
    let row = f
        .rows
        .iter()
        .find(|r| r.context == [3; 32] && r.role == 1 && r.ordinal == 25)
        .context("B25")?;
    let req = oracle::Request::new(0, 1, 25)?;
    let live = json!({"format":2,"role":1,"ordinal":25,"id":state::hex(&row.event),"request":state::hex(&row.request),"text":req.body()});
    assert!(
        f.early
            .b25(&f.model.ledger, 0, &live, duplicate.bound.mark)
            .is_err()
    );
    Ok(())
}
#[test]
fn s54_b1b_partial_duplicate_tap_or_later_graph_owner_failure_never_seals() -> Result<()> {
    let f = Fixture::new()?;
    let mut d = f.draft()?;
    d.begin_owner(0, [1; 32], [1, 0])?;
    d.genesis(f.genesis[0].clone())?;
    assert!(d.genesis(f.genesis[0].clone()).is_err());
    assert!(d.finish([&f.graph, &f.graph]).is_err());
    let mut d = f.draft()?;
    d.begin_owner(0, [1; 32], [1, 0])?;
    d.text(0, &f.rows[0])?;
    assert!(d.text(0, &f.rows[0]).is_err());
    assert!(d.finish_owner(&f.graph, f.lineages[0].clone()).is_err());
    let mut d = f.draft()?;
    f.consume(&mut d)?;
    let mut bad = f.graph.clone();
    bad.contexts[0].records_commitment = [99; 32];
    assert!(d.finish([&f.graph, &bad]).is_err());
    let fail: Result<((), Accounted)> = seal_read(|| {
        let mut d = f.draft()?;
        f.consume(&mut d)?;
        let _pending = d.finish([&f.graph, &f.graph])?;
        anyhow::bail!("later owner/root validation failed")
    });
    assert!(fail.is_err());
    Ok(())
}
#[test]
fn s54_b1b_genesis_lineage_scope_role_slot_and_account_substitution_refuse() -> Result<()> {
    let mut f = Fixture::new()?;
    let original = f.lineages.clone();
    for field in 0..7 {
        f.lineages = original.clone();
        let l = &mut f.lineages[0][0];
        match field {
            0 => l.owner = 1,
            1 => l.slot = 0,
            2 => l.context = [88; 32],
            3 => l.genesis[0] = [89; 32],
            4 => l.offer = [90; 32],
            5 => l.source = [91; 32],
            _ => l.curve = original[1][0].curve,
        };
        assert!(f.run().is_err(), "lineage {field}");
    }
    f.lineages = original;
    let g = f.genesis[0].clone();
    f.genesis[0].event = [99; 32];
    assert!(f.run().is_err());
    f.genesis[0] = g;
    let mut d = f.draft()?;
    assert!(d.begin_owner(0, [1; 32], [0, 1]).is_err());
    Ok(())
}
#[test]
fn s54_b1b_zero_restart_same_pid_new_lifetime_preserves_prior_p10_decrypt() -> Result<()> {
    let mut f = Fixture::new()?;
    let first = f.run()?;
    let prior = first.ledger.closed[5].clone();
    assert!(prior.counts.decrypt == 1 && prior.counts.encrypt == 0);
    let pair = f.restart()?;
    let next = f.run()?;
    assert!(pair[0].key.pid == prior.key.pid && pair[0].key.nonce != prior.key.nonce);
    first.after_zero_restart(&next, &pair)?;
    assert_eq!(next.ledger.closed[5], prior);
    assert!(
        next.ledger
            .closed
            .iter()
            .rev()
            .take(2)
            .all(|c| c.counts == Default::default())
    );
    Ok(())
}
#[test]
fn s54_b1b_restart_reused_nonce_missing_owner_nonzero_or_moved_old_work_refuses() -> Result<()> {
    let mut f = Fixture::new()?;
    let first = f.run()?;
    let pair = f.restart()?;
    let mut next = f.run()?;
    let original = next.ledger.clone();
    for field in 0..6 {
        next.ledger = original.clone();
        let last = next.ledger.closed.len() - 1;
        match field {
            0 => next.ledger.closed[last].key.nonce = first.ledger.closed[0].key.nonce,
            1 => {
                next.ledger.closed.pop();
            }
            2 => next.ledger.closed[last].counts.decrypt = 1,
            3 => next.ledger.closed[5].counts.decrypt = 0,
            4 => next.ledger.starts[last].lifetime = 1,
            _ => next.ledger.closed[last]
                .prefix
                .entries
                .push(first.ledger.closed[0].prefix.entries[0]),
        };
        assert!(
            first.after_zero_restart(&next, &pair).is_err(),
            "restart {field}"
        );
    }
    let mut wrong = pair.clone();
    wrong[1] = wrong[0].clone();
    next.ledger = original;
    assert!(first.after_zero_restart(&next, &wrong).is_err());
    Ok(())
}
#[test]
fn s54_b1b_checked_union_refuses_overflow_and_underflow() -> Result<()> {
    assert!(
        tap::checked_union(
            Work {
                encrypt: u64::MAX,
                decrypt: 0
            },
            Work {
                encrypt: 1,
                decrypt: 0
            },
            Work::default()
        )
        .is_err()
    );
    assert!(
        tap::checked_union(
            Work::default(),
            Work::default(),
            Work {
                encrypt: 0,
                decrypt: 1
            }
        )
        .is_err()
    );
    assert_eq!(
        tap::checked_union(
            Work {
                encrypt: 52,
                decrypt: 52
            },
            Work {
                encrypt: 1304,
                decrypt: 1305
            },
            Work {
                encrypt: 8,
                decrypt: 9
            }
        )?,
        Work {
            encrypt: 1348,
            decrypt: 1348
        }
    );
    Ok(())
}
#[test]
fn s54_b1b_signed_selected_sources_bridge_raw_cipher_body_and_format()
-> super::super::records_fixture::TestResult {
    use super::super::records_fixture as signed;
    let before = kilogram_ratchet::ordered::native_call_counts();
    for (c, author, n) in [
        (0, 0, 17),
        (0, 0, 24),
        (0, 1, 16),
        (0, 1, 24),
        (0, 1, 25),
        (1, 0, 1),
    ] {
        let f = signed::Fixture::new(c)?;
        let case = f.case(author, n, 2)?;
        let r = f.check(&case)?;
        let a = Atom::record(c, &r);
        assert!(
            a.raw == digest(&case.raw)
                && a.cipher
                    == digest(
                        old::decode(&case.raw, kilogram_protocol::ordered_dialog_v2::MAX_EVENT)?
                            .fields(5)?[3]
                            .bytes()?
                    )
                && a.body == Some(body_hash(&case.body))
                && a.format == if c == 0 && n >= 25 { 2 } else { 1 }
        );
        if r.format == 1 {
            let event =
                old::Event::parse(&case.raw, f.actors.devices[author as usize].device_id())?;
            let original = Atom::legacy(c, &case.raw, &event, Some(&case.body))?;
            assert_eq!(a, original);
            for field in 0..3 {
                let mut bad = original.clone();
                match field {
                    0 => bad.raw = [90; 32],
                    1 => bad.cipher = [91; 32],
                    _ => bad.body = Some([92; 32]),
                };
                assert_ne!(a, bad);
            }
        } else {
            let mut bad = case.raw.clone();
            let i = bad.len() - 1;
            bad[i] ^= 1;
            assert!(f.check(&signed::Case { raw: bad, ..case }).is_err());
        }
    }
    assert_eq!(before, kilogram_ratchet::ordered::native_call_counts());
    Ok(())
}
