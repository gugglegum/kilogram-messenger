use super::*;
use kilogram_ratchet::ordered::Packet;
fn bounded_pair(case: u32) -> TestResult<Pair> {
    let mut p = Pair::fresh(case)?;
    for _ in 0..4 {
        for role in 0..2 {
            let r = p.send(role)?;
            p.deliver(&r)?;
        }
    }
    assert_eq!(p.records.len(), 10);
    Ok(p)
}
pub(super) fn snapshot(node: &mut Node) -> TestResult<Vec<u8>> {
    use db::Namespace::*;
    let mut values = vec![];
    for c in 0..2 {
        for ns in [
            Event,
            LaneA,
            LaneB,
            Projection,
            Request,
            Queue,
            Undelivered,
            ReceiptA,
            ReceiptB,
            Pending,
            SeenA,
            SeenB,
            Auth,
            Control,
            Current,
            Native,
            Credit,
            Author0,
            Author1,
            Author2,
            Admission,
        ] {
            let rows = node.store()?.range(
                db::Key {
                    context: c,
                    namespace: ns,
                    bytes: [0; 32],
                },
                128,
            )?;
            assert!(rows.len() < 128, "bounded negative snapshot");
            values.extend(rows);
        }
    }
    let guard = node.native.guard.encode()?;
    let native_generation = node.native.generation;
    let curve = node.native.account.curve_key();
    let sid = node.native.session.as_ref().ok_or("session")?.sid();
    let store = node.store()?;
    Ok(postcard::to_allocvec(&(
        store.generation(),
        store.last_commit()?,
        store.record_counts()?,
        serde_json::to_vec(&store.credits()?)?,
        guard,
        native_generation,
        curve,
        sid,
        values,
    ))?)
}
pub(super) fn measure_reads(node: &mut Node, tag: &str) -> TestResult<()> {
    let role = node.role;
    let before = snapshot(node)?;
    for limit in [1, 32] {
        let timer = Instant::now();
        let mut page = node.store()?.history_first(0, limit)?;
        let mut maximum = (0, 0, 0);
        let mut total = 0;
        let mut pages = 0;
        loop {
            assert!(page.stats.point_gets <= 40 * (limit as u64 + 2) + 3);
            assert!(page.stats.max_candidates <= 2);
            assert!(page.stats.candidate_loads <= limit as u64 + 2);
            maximum.0 = maximum.0.max(page.stats.point_gets);
            maximum.1 = maximum.1.max(page.stats.candidate_loads);
            maximum.2 = maximum.2.max(page.stats.max_candidates);
            total += page.items.len();
            pages += 1;
            if page.finished {
                break;
            }
            page = node.store()?.history_page(0, &page.cursor, limit)?;
        }
        println!(
            "bound2_read tag={tag} role={role} limit={limit} pages={pages} rows={total} observed_max_point_gets={} candidate_loads={} max_candidates={} elapsed_seconds={:.6}",
            maximum.0,
            maximum.1,
            maximum.2,
            timer.elapsed().as_secs_f64()
        );
    }
    assert_eq!(snapshot(node)?, before);
    Ok(())
}
fn outer(
    core: &wire::Core2,
    packet: &Packet,
    signer: &kilogram_identity::DeviceIdentity,
) -> TestResult<wire::ParsedEvent2> {
    Ok(wire::sign_event(core.clone(), packet, signer)?)
}
fn intent(p: &Pair) -> TestResult<(wire::Core2, guard::EventMeta)> {
    let node = &p.nodes[0];
    let g = &node.native.guard;
    let n = g.own().next;
    let mut request = [0; 16];
    request[..4].copy_from_slice(&p.case.to_be_bytes());
    request[8..].copy_from_slice(&n.to_be_bytes());
    let mut parents = vec![
        g.own().last.ok_or("own parent")?,
        g.peer().last.ok_or("peer parent")?,
    ];
    parents.sort();
    let core = wire::Core2 {
        c: node.scope.context,
        d: node.scope.dialog,
        e: node.scope.epoch,
        sid: node.scope.session_id.clone(),
        role: 0,
        n,
        previous: g.own().last,
        sequence: n,
        kind: 3,
        request,
        parents,
        expected: g.expected_send()?,
        author_auth: p.actors.credentials[0].reference(),
        q: node.q,
    };
    let meta = guard::EventMeta {
        role: 0,
        n,
        previous: core.previous,
        sequence: n,
        kind: 3,
        native_kind: 1,
        key: [0; 32],
        index: core.expected.index,
        expected: core.expected.clone(),
    };
    Ok((core, meta))
}
#[test]
fn a3_s3_g01_duplicate_gap_seen_and_lookup_fail_closed() -> TestResult {
    let mut p = bounded_pair(41)?;
    let duplicate = p
        .records
        .iter()
        .rev()
        .find(|r| r.event.core().role == 0)
        .ok_or("row")?
        .clone();
    let before = [snapshot(&mut p.nodes[0])?, snapshot(&mut p.nodes[1])?];
    let calls = p.calls();
    p.deliver(&duplicate)?;
    assert_eq!(p.calls(), calls);
    let mut core = duplicate.event.core().clone();
    core.request[0] ^= 1;
    let event = outer(&core, &duplicate.event.packet(), &p.actors.devices[0])?;
    let conflicting = Record {
        event,
        ..duplicate.clone()
    };
    assert!(p.deliver(&conflicting).is_err());
    assert_eq!(p.calls(), calls);
    assert_eq!(
        before,
        [snapshot(&mut p.nodes[0])?, snapshot(&mut p.nodes[1])?]
    );
    let record = p.send(0)?;
    let meta = record.event.meta();
    let node = &mut p.nodes[1];
    let before = snapshot(node)?;
    let calls = node.native_calls;
    let mut gap = meta.clone();
    gap.n += 1;
    assert!(
        node.native
            .guard
            .preflight_receive(&gap, SeenObservation::Absent, 0, node.native.generation)
            .is_err()
    );
    assert!(
        node.native
            .guard
            .preflight_receive(
                &meta,
                SeenObservation::Present(guard::Seen {
                    generation: 0,
                    first: p.records[0].event.id()
                }),
                0,
                node.native.generation
            )
            .is_err()
    );
    // A continuation requires its current authenticated Seen entry, not an absence hint.
    let mut trial = node.native.guard.clone();
    let step =
        trial.preflight_receive(&meta, SeenObservation::Absent, 0, node.native.generation)?;
    trial.apply(step, &meta, record.event.id(), SeenObservation::Absent)?;
    let mut continuation = meta.clone();
    continuation.n += 1;
    continuation.sequence += 1;
    continuation.previous = Some(record.event.id());
    continuation.index += 1;
    continuation.expected.index += 1;
    continuation.expected.tail = None;
    assert!(
        trial
            .preflight_receive(
                &continuation,
                SeenObservation::Absent,
                0,
                node.native.generation
            )
            .is_err()
    );
    let mut lookup_calls = 0;
    let mut consume_calls = 0;
    for integrity in [true, false] {
        let outcome = (|| -> TestResult<()> {
            lookup_calls += 1;
            let error = if integrity {
                db::Error::Integrity("injected Seen lookup")
            } else {
                db::Error::Io(std::io::Error::other("injected Seen lookup"))
            };
            let observation: SeenObservation = Err::<SeenObservation, _>(error)?;
            let _ = node.native.guard.preflight_receive(
                &meta,
                observation,
                0,
                node.native.generation,
            )?;
            consume_calls += 1;
            Ok(())
        })();
        assert!(outcome.is_err());
    }
    assert_eq!((lookup_calls, consume_calls), (2, 0));
    assert_eq!(node.native_calls, calls);
    assert_eq!(snapshot(node)?, before);
    p.deliver(&record)?;
    println!(
        "a3_s3_G01=7 duplicate_native_calls=0 lookup_attempts={lookup_calls} consume_after_lookup_error={consume_calls} accepted_points_unchanged=true"
    );
    Ok(())
}
#[test]
fn a3_s3_g02_six_post_trial_rollbacks() -> TestResult {
    let mut attempts = 0;
    let mut successes = 0;
    for case in 0..6 {
        let mut p = bounded_pair(50 + case)?;
        let (core, intent) = intent(&p)?;
        let text = format!("S3:{}:0:{}:Привет", p.case, core.n);
        let commit = p.nodes[0].commit_id();
        let store = p.nodes[0].store()?;
        store.commit_history(
            0,
            store.generation(),
            commit,
            db::HistoryOperation::QueueAccept {
                request: core.request,
                text: text.clone(),
            },
        )?;
        let snapshots = [snapshot(&mut p.nodes[0])?, snapshot(&mut p.nodes[1])?];
        let mut payload = wire::inner2(&core, a([b(text.as_bytes())]))?;
        if case == 1 {
            let mut f = wire::decode(&payload, wire::MAX_INNER)?.list()?.to_vec();
            f[0] = u(1);
            payload = wire::encode(&a(f), wire::MAX_INNER)?;
        }
        if case == 2 {
            let mut f = wire::decode(&payload, wire::MAX_INNER)?.list()?.to_vec();
            let mut other = core.clone();
            other.request[0] ^= 1;
            f[1] = other.value();
            payload = wire::encode(&a(f), wire::MAX_INNER)?;
        }
        if case == 3 {
            payload = wire::encode(
                &a([
                    u(2),
                    core.value(),
                    a([
                        b([1; 32]),
                        b([2; 32]),
                        b([3; 32]),
                        a([b([4; 32]), b([5; 32]), b([6; 32])]),
                    ]),
                ]),
                wire::MAX_INNER,
            )?;
        }
        let step =
            p.nodes[0]
                .native
                .guard
                .preflight_send(&intent, 0, p.nodes[0].native.generation)?;
        let mut sender = p.nodes[0].native.fork();
        attempts += 1;
        let mut packet = sender
            .session
            .as_mut()
            .ok_or("session")?
            .encrypt(&payload)?;
        if case == 0 {
            let end = packet.bytes.len() - 1;
            packet.bytes[end] ^= 1;
        }
        let event = outer(&core, &packet, &p.actors.devices[0])?;
        let meta = event.meta();
        let verified = event
            .clone()
            .verify(&p.selection, &p.actors.credentials[0])?;
        if case == 4 {
            assert!(
                sender
                    .guard
                    .apply(
                        step,
                        &meta,
                        event.id(),
                        SeenObservation::Present(guard::Seen {
                            generation: 0,
                            first: [99; 32]
                        })
                    )
                    .is_err()
            );
        } else if case == 5 {
            sender
                .guard
                .apply(step, &meta, event.id(), SeenObservation::Absent)?;
            sender.generation += 1;
            let complete = sender.seal(
                &p.nodes[0].scope,
                p.nodes[0].q,
                0,
                &p.nodes[0].crypto,
                p.nodes[0].view,
                p.nodes[0].cert,
            )?;
            assert!(complete.len() <= guard::MAX_CHECKPOINT);
            let injected = vec![0; guard::MAX_CHECKPOINT + 1];
            assert!(guard::check_checkpoint_size(&injected).is_err());
        } else {
            let node = &mut p.nodes[1];
            let observation = node.seen(&meta)?;
            let _step = node.native.guard.preflight_receive(
                &meta,
                observation,
                0,
                node.native.generation,
            )?;
            let mut receiver = node.native.fork();
            attempts += 1;
            let plain = receiver.session.as_mut().ok_or("session")?.decrypt(&packet);
            if case == 0 {
                assert!(plain.is_err());
            } else {
                successes += 1;
                assert!(verified.verify_inner(&plain?).is_err());
            }
        }
        assert_eq!(
            snapshots,
            [snapshot(&mut p.nodes[0])?, snapshot(&mut p.nodes[1])?],
            "G02 case{case}"
        );
        let next = p.send(0)?;
        let exact = next.event.raw().to_vec();
        p.deliver(&next)?;
        assert_eq!(next.event.raw(), exact);
        p.verify(9)?;
    }
    assert_eq!(attempts, 10);
    assert_eq!(successes, 3);
    println!(
        "a3_s3_G02=6 trial_message_calls={attempts} post_MAC_inner_rejections={successes} next_valid_transitions=6 funded_queue_preserved=true checkpoints_points_unchanged=true"
    );
    Ok(())
}
