use super::*;
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
#[test]
fn a3_s3_arithmetic_24_vectors() -> TestResult {
    let m = u64::MAX;
    let mut count = 0;
    assert_eq!(consume_index(m - 1)?, m);
    count += 1;
    assert!(consume_index(m).is_err());
    count += 1;
    assert_eq!(continuation_index(m - 2)?, (m - 1, m));
    count += 1;
    assert!(continuation_index(m - 1).is_err());
    count += 1;
    assert_eq!(dh_step(m - 1, 1, (m - 1) / 2)?, m);
    count += 1;
    assert!(dh_step(m, 0, 0).is_err());
    count += 1;
    assert_eq!(turn(0, (m - 1) / 2)?, m - 1);
    count += 1;
    assert_eq!(turn(1, (m - 1) / 2)?, m);
    count += 1;
    for role in 0..=1 {
        assert!(turn(role, (m - 1) / 2 + 1).is_err());
        count += 1;
    }
    for kind in [
        Counter::Ordinal,
        Counter::Author,
        Counter::RootGeneration,
        Counter::NativeGeneration,
        Counter::LaneGeneration,
    ] {
        assert_eq!(successor(m - 1, kind)?, m);
        assert!(successor(m, kind).is_err());
        count += 2;
    }
    // Symmetric continuations at a maximal proven sender T do not advance that T.
    let g_max = (m - 1) / 2;
    let lane = |key| Lane {
        next: g_max + 2,
        last: Some([key; 32]),
        g: g_max,
        key: Some([key; 32]),
        index: 0,
        first: Some([key; 32]),
        parent: Some([key - 1; 32]),
        sequence: Some(g_max + 1),
    };
    let maximal = Guard2 {
        version: 2,
        role: 1,
        own: lane(20),
        peer: lane(10),
        rotate_due: false,
        received_new_first: Some([10; 32]),
        used_own_first: Some([19; 32]),
    };
    for receive in [false, true] {
        let l = if receive {
            maximal.peer()
        } else {
            maximal.own()
        };
        let e = EventMeta {
            role: if receive { 0 } else { 1 },
            n: l.next,
            previous: l.last,
            sequence: g_max + 2,
            kind: 3,
            native_kind: 1,
            key: l.key.ok_or("key")?,
            index: 1,
            expected: Expected {
                g: g_max,
                index: 1,
                parent: l.parent,
                tail: None,
            },
        };
        let step = if receive {
            maximal.preflight_receive(
                &e,
                SeenObservation::Present(Seen {
                    generation: g_max,
                    first: l.first.ok_or("first")?,
                }),
                0,
                0,
            )?
        } else {
            maximal.preflight_send(&e, 0, 0)?
        };
        assert_eq!(step.native_post_turn, m);
        assert_eq!(step.native_post_index, 2);
        count += 1;
    }
    let bounded_lane = |key, parent| Lane {
        next: 1,
        last: Some([key; 32]),
        g: 0,
        key: Some([key; 32]),
        index: 0,
        first: Some([key; 32]),
        parent,
        sequence: Some(0),
    };
    let mut g = Guard2 {
        version: 2,
        role: 0,
        own: bounded_lane(10, None),
        peer: bounded_lane(20, Some([10; 32])),
        rotate_due: true,
        received_new_first: Some([20; 32]),
        used_own_first: Some([10; 32]),
    };
    let incoming = EventMeta {
        role: 1,
        n: 1,
        previous: Some([20; 32]),
        sequence: 1,
        kind: 3,
        native_kind: 1,
        key: [30; 32],
        index: 0,
        expected: Expected {
            g: 1,
            index: 0,
            parent: Some([10; 32]),
            tail: Some(g.peer.tail()?),
        },
    };
    assert!(matches!(
        g.preflight_receive(&incoming, SeenObservation::Absent, 0, 0),
        Err(Error::Input("new peer DH in Inactive"))
    ));
    count += 1;
    let intent = meta(0, 1, 30, g.expected_send()?);
    g.used_own_first = Some([99; 32]);
    assert!(matches!(
        g.preflight_send(&intent, 0, 0),
        Err(Error::Checkpoint("native Active/Inactive provenance"))
    ));
    count += 1;
    assert_eq!(count, 24);
    println!("a3_s3_arithmetic_vectors={count} native_calls=0");
    Ok(())
}
fn meta(role: u8, n: u64, key: u8, expected: Expected) -> EventMeta {
    EventMeta {
        role,
        n,
        previous: if n == 0 { None } else { Some([key - 1; 32]) },
        sequence: n,
        kind: if n == 0 { role + 1 } else { 3 },
        native_kind: if role == 0 && n == 0 { 0 } else { 1 },
        key: [key; 32],
        index: expected.index,
        expected,
    }
}
#[test]
fn a3_s3_phase_checkpoint_and_provenance() -> TestResult {
    let mut guards = [Guard2::new(0)?, Guard2::new(1)?];
    let mut seen = std::collections::BTreeMap::new();
    let mut trace = vec![];
    let mut phases = 0;
    for (role, key) in [(0, 10), (1, 20), (0, 30), (1, 40)] {
        let role = role as usize;
        let mut e = meta(
            role as u8,
            guards[role].own.next,
            key,
            guards[role].expected_send()?,
        );
        e.previous = guards[role].own.last;
        let obs = SeenObservation::Absent;
        let send = guards[role].preflight_send(&e, 0, 0)?;
        let receive = guards[1 - role].preflight_receive(&e, obs, 0, 0)?;
        assert_eq!(send.native_post_turn, receive.native_post_turn);
        assert_eq!(send.native_post_index, 1);
        let id = [key; 32];
        guards[role].apply(send, &e, id, obs)?;
        guards[1 - role].apply(receive, &e, id, obs)?;
        seen.insert(
            (e.role, e.key),
            Seen {
                generation: e.expected.g,
                first: id,
            },
        );
        trace.push((e, id));
        phases += 2;
    }
    assert_eq!(phases, 8);
    for (role, guard) in guards.iter().enumerate() {
        let bytes = guard.encode()?;
        assert!(bytes.len() <= MAX_GUARD);
        assert_eq!(Guard2::restore(&bytes, role as u8)?, *guard);
        assert_eq!(import_l1(role as u8, &trace, &bytes)?.guard, *guard);
        let mut damaged = bytes.clone();
        damaged.push(0);
        assert!(Guard2::restore(&damaged, role as u8).is_err());
    }
    let before = guards[0].encode()?;
    let mut bad = trace[3].0.clone();
    bad.n = guards[0].peer.next;
    bad.previous = guards[0].peer.last;
    bad.sequence += 1;
    bad.key = [99; 32];
    bad.expected.g += 1;
    assert!(
        guards[0]
            .preflight_receive(&bad, SeenObservation::Absent, 0, 0)
            .is_err()
    );
    assert_eq!(guards[0].encode()?, before);
    println!(
        "a3_s3_phase_vectors={phases} guard_bytes={} import_sources={}",
        before.len(),
        trace.len()
    );
    Ok(())
}
#[test]
fn a3_s3_public_config_serde_rejects_disabled_v2() -> TestResult {
    // Only the public configuration type, never a private SessionPickle representation.
    let config = vodozemac::olm::SessionConfig::version_1();
    let raw = postcard::to_allocvec(&config)?;
    assert_eq!(
        postcard::from_bytes::<vodozemac::olm::SessionConfig>(&raw)?,
        config
    );
    assert!(postcard::from_bytes::<vodozemac::olm::SessionConfig>(&[1]).is_err());
    println!(
        "a3_s3_public_config_v2_serde_rejection=1 native_message_calls=0 actual_config2_pickle=false"
    );
    Ok(())
}
