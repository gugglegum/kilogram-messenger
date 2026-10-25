//! Deterministic validation of authenticated readable metadata, never Native replay.
use super::super::ratchet::{Guard, Lane};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::{Event, Id};
use kilogram_ratchet::ordered_v2 as g2;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(super) struct ReadableMeta {
    pub id: Id,
    pub event: g2::EventMeta,
    pub parents: Vec<Id>,
    pub request: [u8; 16],
}
impl ReadableMeta {
    pub fn of(event: &Event) -> Result<Self> {
        let c = &event.core;
        Ok(Self {
            id: event.id()?,
            parents: c.parents.clone(),
            request: c.request,
            event: g2::EventMeta {
                role: c.role.try_into()?,
                n: c.n,
                previous: c.previous,
                sequence: c.sequence,
                kind: c.kind.try_into()?,
                native_kind: event.native_kind.try_into()?,
                key: event.key,
                index: event.index,
                expected: g2::Expected {
                    g: c.expected.g,
                    index: c.expected.index,
                    parent: c.expected.parent,
                    tail: c.expected.tail.as_ref().map(|t| g2::ClosedTail {
                        g: t.g,
                        key: t.key,
                        index: t.index,
                        last: t.last,
                    }),
                },
            },
        })
    }
}
fn lane(old: &Lane) -> g2::Lane {
    g2::Lane {
        next: old.next,
        last: old.last,
        g: old.g,
        key: old.key,
        index: old.index,
        first: old.first,
        parent: old.parent,
        sequence: old.sequence,
    }
}
pub(super) fn fold(old: &Guard, lanes: &[Vec<ReadableMeta>; 2]) -> Result<g2::ImportedL1> {
    old.validate(old.role)?;
    ensure!(
        lanes.iter().map(Vec::len).sum::<usize>() <= 1024,
        "legacy readable count"
    );
    let role = u8::try_from(old.role)?;
    let expected = g2::compact_l1_checkpoint(g2::L1CheckpointFacts {
        role,
        own: lane(&old.own),
        peer: lane(&old.peer),
        rotate_due: old.rotate_due,
        received_new_first: old.received_new_first,
        used_own_first: old.used_own_first,
    })?;
    for (r, items) in lanes.iter().enumerate() {
        let next = if r == usize::from(role) {
            old.own.next
        } else {
            old.peer.next
        };
        ensure!(items.len() as u64 == next, "readable lane completeness");
        for (n, m) in items.iter().enumerate() {
            ensure!(
                m.event.role as usize == r && m.event.n == n as u64,
                "lane bijection"
            );
        }
    }
    let mut catalog = HashMap::new();
    for m in lanes.iter().flatten() {
        ensure!(
            catalog.insert(m.id, m.event.kind).is_none(),
            "duplicate readable ID"
        );
    }
    let mut current = g2::Guard2::new(role)?;
    let mut pos = [0usize; 2];
    let mut seen = HashMap::<(u8, Id), g2::Seen>::new();
    let mut emitted = HashSet::new();
    let mut ordered = Vec::with_capacity(catalog.len());
    struct Candidate {
        r: usize,
        guard: g2::Guard2,
        continuation: bool,
    }
    while pos != [lanes[0].len(), lanes[1].len()] {
        let mut candidates = Vec::with_capacity(2);
        for r in 0..2 {
            let Some(m) = lanes[r].get(pos[r]) else {
                continue;
            };
            let e = &m.event;
            let l = if r == usize::from(role) {
                current.own()
            } else {
                current.peer()
            };
            ensure!(
                e.n == l.next && e.previous == l.last,
                "source readable prefix"
            );
            for parent in &m.parents {
                ensure!(
                    catalog.get(parent) == Some(&3),
                    "user parent must be readable Text"
                );
            }
            if m.parents.iter().any(|p| !emitted.contains(p))
                || e.expected.parent.is_some_and(|p| !emitted.contains(&p))
                || e.expected
                    .tail
                    .as_ref()
                    .is_some_and(|t| !emitted.contains(&t.last))
            {
                continue;
            }
            let observation = seen
                .get(&(e.role, e.key))
                .copied()
                .map_or(g2::SeenObservation::Absent, g2::SeenObservation::Present);
            let mut trial = current.clone();
            let step = if r == usize::from(role) {
                trial.preflight_send(e, 0, 0)
            } else {
                trial.preflight_receive(e, observation, 0, 0)
            };
            let Ok(step) = step else { continue };
            if trial.apply(step, e, m.id, observation).is_err() {
                continue;
            }
            let continuation = l.next > 0
                && l.key == Some(e.key)
                && l.g == e.expected.g
                && e.expected.tail.is_none();
            candidates.push(Candidate {
                r,
                guard: trial,
                continuation,
            });
        }
        let selected = if candidates.iter().any(|c| c.continuation) {
            candidates
                .iter()
                .position(|c| c.continuation && c.r == usize::from(role))
                .or_else(|| candidates.iter().position(|c| c.continuation))
                .context("continuation selection")?
        } else {
            ensure!(
                candidates.len() == 1,
                "legacy fold no progress or inconsistent native firsts"
            );
            0
        };
        let take = candidates.swap_remove(selected);
        let m = &lanes[take.r][pos[take.r]];
        current = take.guard;
        pos[take.r] += 1;
        seen.entry((m.event.role, m.event.key)).or_insert(g2::Seen {
            generation: m.event.expected.g,
            first: m.id,
        });
        ensure!(emitted.insert(m.id), "readable ID repeated");
        ordered.push((m.event.clone(), m.id));
    }
    ensure!(current.encode()? == expected, "exact final Guard1 summary");
    for r in 0..2u8 {
        let old_seen = if r == role {
            &old.own.seen
        } else {
            &old.peer.seen
        };
        ensure!(
            seen.keys().filter(|(rr, _)| *rr == r).count() == old_seen.len()
                && old_seen.iter().all(|key| seen.contains_key(&(r, *key))),
            "exact Guard1 seen set"
        );
    }
    let imported = g2::import_l1(role, &ordered, &expected)?;
    ensure!(
        imported
            .seen
            .iter()
            .all(|(r, k, v)| seen.get(&(*r, *k)) == Some(v)),
        "exact seen generation/first"
    );
    Ok(imported)
}
