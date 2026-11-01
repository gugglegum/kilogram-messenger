//! Full geometry uses synthetic normalized metadata, never a signature/Native claim.
//! Only the small bridge passes real signed events/receipts through A3 verification.
use super::super::records_fixture as fixture;
use super::*;
use records::Record;

fn scopes() -> [Scope; 2] {
    std::array::from_fn(|c| Scope {
        context: [c as u8 + 1; 32],
        d: [c as u8 + 3; 32],
        e: [c as u8 + 5; 32],
        sid: "S".repeat(43),
        selected: [c as u8 + 7; 32],
        certs: [[21; 32], [22; 32]],
        controls: [[23; 32], [24; 32], [c as u8 + 30; 32], [c as u8 + 32; 32]],
        origin: [c as u8 + 40; 32],
    })
}
fn id(c: usize, role: u8, n: u16) -> old::Id {
    let mut value = [0; 32];
    value[0] = c as u8 + 10;
    value[1..3].copy_from_slice(&n.to_be_bytes());
    value[3] = role;
    value
}
fn rows(scopes: &[Scope; 2]) -> Result<Vec<Record>> {
    let mut rows = Vec::with_capacity(TEXTS);
    for (c, s) in scopes.iter().enumerate() {
        for role in 0..2 {
            for n in 1..=limit(c) {
                let previous = if n == 1 {
                    s.genesis(role as usize)
                } else {
                    id(c, role, n - 1)
                };
                rows.push(Record {
                    format: if c == 0 && n >= 25 { 2 } else { 1 },
                    event: id(c, role, n),
                    context: s.context,
                    d: s.d,
                    e: s.e,
                    sid: s.sid.clone(),
                    selection: s.selected,
                    role,
                    ordinal: u64::from(n),
                    request: source::parse_request_id(&source::request_id(oracle::Request::new(
                        c as u8, role, n,
                    )?))?,
                    previous: Some(previous),
                    parents: if n == 1 { vec![] } else { vec![previous] },
                    // Equal opposite-author sequences are valid, not a global clock.
                    sequence: u64::from(n),
                    author_auth: [50; 32],
                    source_hash: [51; 32],
                    ciphertext_hash: [52; 32],
                    body_hash: [53; 32],
                    receipt_format: 2,
                    receipt_hash: [54; 32],
                    receiver_auth: [55; 32],
                });
            }
        }
    }
    Ok(rows)
}
fn complete(scopes: [Scope; 2], rows: Vec<Record>) -> Result<Terminal> {
    let mut graph = Graph::synthetic(scopes)?;
    for row in rows {
        graph.insert(row)?;
    }
    graph.finish()
}
fn change(
    rows: &mut [Record],
    c: usize,
    role: u8,
    n: u16,
    f: impl FnOnce(&mut Record),
) -> Result<()> {
    let row = rows
        .iter_mut()
        .find(|r| r.event == id(c, role, n))
        .context("synthetic row")?;
    f(row);
    Ok(())
}

#[test]
fn s54_p11a4_complete_geometry_shuffle_and_history_tie_break() -> Result<()> {
    let s = scopes();
    let original = rows(&s)?;
    let terminal = complete(s.clone(), original.clone())?;
    ensure!(
        terminal.texts == 1344 && terminal.genesis == 4,
        "Text and genesis counts"
    );
    for c in 0..2 {
        let t = &terminal.contexts[c];
        ensure!(
            t.count == usize::from(limit(c)) * 2 && t.next == [u64::from(limit(c)) + 1; 2],
            "all four terminal lanes"
        );
        ensure!(
            t.tails == [id(c, 0, limit(c)), id(c, 1, limit(c))],
            "actual derived tails"
        );
        let expected = (1..=limit(c))
            .flat_map(|n| [id(c, 0, n), id(c, 1, n)])
            .collect::<Vec<_>>();
        // At each n both lane heads have consumed their own previous parent;
        // ordinal-first IDs give A_n < B_n < A_(n+1), so this is the history rule.
        ensure!(
            t.ordered == expected,
            "eligible EventID tie break, not all A then B"
        );
        terminal.stored(c, t.next, t.tails)?;
    }
    let mut reverse = original.clone();
    reverse.reverse();
    terminal.same(&complete(s.clone(), reverse)?)?;
    let mut shuffled = original;
    shuffled.rotate_left(239);
    for i in (0..shuffled.len() - 1).step_by(3) {
        shuffled.swap(i, i + 1);
    }
    terminal.same(&complete(s, shuffled)?)
}

#[test]
fn s54_p11a4_blocked_lane_head_waits_for_later_inserted_parent() -> Result<()> {
    let s = scopes();
    let mut data = rows(&s)?;
    change(&mut data, 0, 0, 1, |r| r.parents = vec![id(0, 1, 2)])?;
    // A1 waits for B2, B1 has no parent, B2 waits for B1. Then A1 and
    // B3 are eligible and A1 has the smaller ID; next A2 wins over B3.
    let t = complete(s, data)?;
    ensure!(
        t.contexts[0].ordered[..4] == [id(0, 1, 1), id(0, 1, 2), id(0, 0, 1), id(0, 0, 2)],
        "eligibility before ID ordering"
    );
    Ok(())
}

#[test]
fn s54_p11a4_no_prefix_duplicates_collisions_or_capacity_overflow() -> Result<()> {
    let s = scopes();
    let data = rows(&s)?;
    ensure!(
        Graph::synthetic(s.clone())?.finish().is_err(),
        "empty prefix"
    );
    let mut missing = data.clone();
    missing.remove(1279);
    ensure!(
        complete(s.clone(), missing).is_err(),
        "one missing actual position"
    );
    let mut g = Graph::synthetic(s.clone())?;
    g.insert(data[0].clone())?;
    ensure!(g.insert(data[0].clone()).is_err(), "duplicate position");
    let mut duplicate_id = data[1].clone();
    duplicate_id.event = data[0].event;
    ensure!(g.insert(duplicate_id).is_err(), "duplicate actual EventID");
    let mut duplicate_request = data[1].clone();
    duplicate_request.request = data[0].request;
    ensure!(
        g.insert(duplicate_request).is_err(),
        "duplicate actual request"
    );
    for anchor in s.iter().flat_map(|s| [s.genesis(0), s.genesis(1)]) {
        let mut row = data[1].clone();
        row.event = anchor;
        ensure!(g.insert(row).is_err(), "any genesis/Text collision");
    }
    for row in data.iter().skip(1) {
        g.insert(row.clone())?;
    }
    ensure!(g.insert(data[0].clone()).is_err(), "bounded capacity");
    // Rejected inserts must leave all three indexes intact.
    g.finish()?.same(&complete(s, data)?)
}

#[test]
fn s54_p11a4_scope_format_and_bounded_indices_are_enforced() -> Result<()> {
    let s = scopes();
    let data = rows(&s)?;
    let mutations: Vec<Record> = (0..13)
        .map(|i| {
            let mut r = data[0].clone();
            match i {
                0 => r.context = [99; 32],
                1 => r.d = [99; 32],
                2 => r.e = [99; 32],
                3 => r.sid = "X".repeat(43),
                4 => r.selection = [99; 32],
                5 => r.role = 2,
                6 => r.ordinal = 0,
                7 => r.ordinal = 641,
                8 => r.format = 2,
                9 => r.receipt_format = 3,
                10 => r.previous = None,
                11 => r.parents = vec![[1; 32], [1; 32]],
                _ => r.parents = vec![[1; 32], [2; 32], [3; 32]],
            };
            r
        })
        .collect();
    for row in mutations {
        ensure!(
            Graph::synthetic(s.clone())?.insert(row).is_err(),
            "invalid supplied scope/shape"
        );
    }
    let mut unsorted = data[0].clone();
    unsorted.parents = vec![[2; 32], [1; 32]];
    ensure!(
        Graph::synthetic(s.clone())?.insert(unsorted).is_err(),
        "canonical parents"
    );
    let mut c2 = data[1280].clone();
    c2.ordinal = 33;
    ensure!(
        Graph::synthetic(s.clone())?.insert(c2).is_err(),
        "C2 terminal bound"
    );
    let mut current = data[24].clone();
    current.receipt_format = 1;
    ensure!(
        Graph::synthetic(s.clone())?.insert(current).is_err(),
        "new source requires new receipt"
    );
    let mut same_context = s.clone();
    same_context[1].context = same_context[0].context;
    ensure!(Graph::synthetic(same_context).is_err(), "distinct contexts");
    let mut same_genesis = s;
    same_genesis[1].controls[2] = same_genesis[0].genesis(0);
    ensure!(
        Graph::synthetic(same_genesis).is_err(),
        "four distinct original anchors"
    );
    Ok(())
}

#[test]
fn s54_p11a4_actual_genesis_predecessor_and_stored_tails() -> Result<()> {
    let s = scopes();
    let data = rows(&s)?;
    for (c, role, n, previous) in [
        (0, 0, 1, s[0].genesis(1)),
        (0, 1, 2, id(0, 0, 1)),
        (1, 1, 32, id(1, 1, 30)),
    ] {
        let mut bad = data.clone();
        change(&mut bad, c, role, n, |r| r.previous = Some(previous))?;
        ensure!(complete(s.clone(), bad).is_err(), "actual lane predecessor");
    }
    let t = complete(s, data)?;
    ensure!(
        t.stored(0, [640, 641], t.contexts[0].tails).is_err(),
        "actual next"
    );
    ensure!(
        t.stored(0, [641, 641], [id(0, 0, 639), id(0, 1, 640)])
            .is_err(),
        "actual tail"
    );
    ensure!(
        t.stored(2, [33, 33], t.contexts[1].tails).is_err(),
        "stored context bound"
    );
    Ok(())
}

#[test]
fn s54_p11a4_parent_mapping_and_cycles_reject_corrupt_metadata() -> Result<()> {
    let s = scopes();
    let data = rows(&s)?;
    for parent in [[99; 32], id(0, 0, 1), id(1, 0, 1), s[0].genesis(0)] {
        let mut bad = data.clone();
        change(&mut bad, 0, 0, 1, |r| r.parents = vec![parent])?;
        ensure!(
            complete(s.clone(), bad).is_err(),
            "missing/self/foreign/genesis parent"
        );
    }
    // Impossible signed hash cycles are deliberately modeled only as corrupt
    // normalized metadata, then rejected by the actual causal frontier walk.
    let mut cycle = data.clone();
    change(&mut cycle, 0, 0, 1, |r| r.parents = vec![id(0, 1, 1)])?;
    change(&mut cycle, 0, 1, 1, |r| r.parents = vec![id(0, 0, 1)])?;
    ensure!(complete(s.clone(), cycle).is_err(), "cross-lane cycle");
    let mut blocked = data;
    change(&mut blocked, 0, 0, 1, |r| r.parents = vec![id(0, 0, 2)])?;
    ensure!(
        complete(s, blocked).is_err(),
        "own lane successor blocks frontier"
    );
    Ok(())
}

#[test]
fn s54_p11a4_owner_cut_comparison_covers_each_normalized_commitment() -> Result<()> {
    let s = scopes();
    let data = rows(&s)?;
    let expected = complete(s.clone(), data.clone())?;
    for i in 0..9 {
        let mut changed = data.clone();
        change(&mut changed, 0, 0, 1, |r| match i {
            0 => r.source_hash = [61; 32],
            1 => r.ciphertext_hash = [62; 32],
            2 => r.body_hash = [63; 32],
            3 => r.receipt_hash = [64; 32],
            4 => r.sequence = 90,
            5 => r.author_auth = [65; 32],
            6 => r.receiver_auth = [66; 32],
            7 => r.receipt_format = 1,
            _ => r.parents = vec![id(0, 1, 1)],
        })?;
        ensure!(
            expected.same(&complete(s.clone(), changed)?).is_err(),
            "actual normalized commitment change"
        );
    }
    let mut changed_scope = s.clone();
    changed_scope[0].origin = [67; 32];
    ensure!(
        expected
            .same(&complete(changed_scope, data.clone())?)
            .is_err(),
        "authenticated original export commitment"
    );
    let mut changed_scope = s;
    changed_scope[0].certs[0] = [68; 32];
    ensure!(
        expected.same(&complete(changed_scope, data)?).is_err(),
        "scope credential commitment"
    );
    Ok(())
}

#[test]
fn s54_p11a4_a3_signed_record_output_bridges_into_complete_graph() -> fixture::TestResult {
    let before = kilogram_ratchet::ordered::native_call_counts();
    let fixtures = [fixture::Fixture::new(0)?, fixture::Fixture::new(1)?];
    let mut s = scopes();
    for (c, f) in fixtures.iter().enumerate() {
        s[c].context = f.selected.c();
        s[c].d = f.selected.d();
        s[c].e = f.selected.e();
        s[c].sid = f.selected.sid().ok_or("SID")?.into();
        s[c].selected = f.selected.proof().q();
        s[c].certs = f.selected.proof().certs();
        for role in 0..2 {
            s[c].controls[role + 2] = old::hash(old::EVENT_ID, &f.genesis[role]);
        }
    }
    // This private test scope uses actual signed genesis IDs and selected proof,
    // but is not an authenticated VerifiedExistingScope1 export fixture.
    // Remaining geometry stays explicitly synthetic; no full-inventory signature claim.
    let mut data = rows(&s)?;
    for (c, f) in fixtures.iter().enumerate() {
        for role in 0..2 {
            let request = oracle::Request::new(c as u8, role, 1)?;
            let mut core = f.core(role, 1)?;
            core.previous = Some(s[c].genesis(role as usize));
            let case = f.record(request, core, 1, 2, &f.normal)?;
            let record = f.check(&case)?;
            let actual = record.event;
            let position = data
                .iter()
                .position(|r| r.event == id(c, role, 1))
                .ok_or("synthetic bridge position")?;
            data[position] = record;
            change(&mut data, c, role, 2, |r| {
                r.previous = Some(actual);
                r.parents = vec![actual];
            })?;
        }
    }
    let terminal = complete(s, data)?;
    assert_eq!(terminal.texts, 1344);
    assert_eq!(terminal.genesis, 4);
    assert_eq!(before, kilogram_ratchet::ordered::native_call_counts());
    Ok(())
}
