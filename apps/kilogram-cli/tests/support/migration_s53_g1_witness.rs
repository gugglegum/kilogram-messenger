//! Original-only G1 witnesses. No transition, admission or runtime construction.
use super::{Node, s53_deadline};
use anyhow::{Context, Result, ensure};
use kilogram_cli::ordered_dialog::{
    handshake::Spec,
    runtime7::Profile3,
    state::{self, Head},
};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

// Reuse the pinned Cold-B pure verifiers and genuine two-Text fixture verbatim.
// G1 never calls readers::b0/full or the private-data preparation entrypoints.
#[allow(dead_code)]
#[path = "s53_cold/fixed.rs"]
mod fixed;
#[allow(dead_code)]
#[path = "s53_cold/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "s53_cold/history.rs"]
mod history;
#[allow(dead_code)]
#[path = "s53_cold/progress.rs"]
mod progress;
#[allow(dead_code)]
#[path = "s53_cold/readers.rs"]
mod readers;

struct Anchor {
    raw: Vec<u8>,
    offer: old::Id,
    grant: old::Id,
}
impl Anchor {
    fn capture(repo: &mut store::L1Repository, local: usize) -> Result<Self> {
        let scope = readers::scope(repo, 0, local)?;
        Ok(Self {
            raw: scope.raw,
            offer: scope.head.offer.context("original Offer reference")?,
            grant: scope.head.grant.context("original Grant reference")?,
        })
    }
    fn verify(&self, repo: &mut store::L1Repository) -> Result<wire::VerifiedExistingScope1> {
        // Use saved original control references, never a current Native or
        // current L1-only admission predicate after conversion to Active.
        let mut genesis = Vec::new();
        let mut ready = Vec::new();
        for role in 0..2 {
            genesis.push(old::b(
                repo.event_at(0, role, 0)?.context("original genesis")?,
            ));
            ready.push(old::b(
                repo.control(0, store::L1Control::Ready(role))?
                    .context("original Ready")?,
            ));
        }
        let raw = old::encode(
            &old::a([
                old::u(1),
                old::b(repo.spec(0)?),
                old::b(repo.signed_control(0, self.offer)?),
                old::b(repo.signed_control(0, self.grant)?),
                old::b(
                    repo.control(0, store::L1Control::InitialPair)?
                        .context("original pair")?,
                ),
                old::a(genesis),
                old::a(ready),
            ]),
            wire::MAX_EXISTING_ORIGIN,
        )?;
        ensure!(raw == self.raw, "original signed scope bytes changed");
        Ok(wire::VerifiedExistingScope1::verify_export(&raw)?)
    }
}

#[derive(PartialEq)]
struct Records {
    digest: String,
    history: BTreeMap<String, Value>,
    sources: usize,
    own_requests: usize,
}
fn records(
    repo: &mut store::L1Repository,
    expected: &fixed::Expected,
    anchor: &Anchor,
) -> Result<Records> {
    ensure!(
        expected.next.iter().all(|n| (1..=512).contains(n)),
        "original source cap"
    );
    let scope = anchor.verify(repo)?;
    expected.scope(&scope)?;
    let mut digest = readers::Digest::new(b"kg:s53:G1:original-records:1\0");
    digest.bytes(&anchor.raw);
    let mut rows = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut own_requests = 0;
    for role in 0..2 {
        let mut previous = None;
        for n in 0..expected.next[role] {
            s53_deadline::check()?;
            let raw = repo
                .event_at(0, role as u8, n)?
                .context("original source missing")?;
            let parsed = old::Event::parse(&raw, scope.peers()[role].certificate().device_id())?;
            let (auth, _) = readers::credential(repo, parsed.core.author_auth)?;
            let verified = readers::verified_source(&scope, &raw, &auth, role, n)?;
            let e = verified.event();
            let id = e.id()?;
            ensure!(
                ids.insert(id) && e.core.previous == previous,
                "original source chain/uniqueness"
            );
            previous = Some(id);
            digest.bytes(&raw);
            digest.bytes(&auth);
            if n == 0 {
                continue;
            }
            let (request, text) = expected.text(role, n);
            ensure!(
                e.core.request == request,
                "original signed request convention"
            );
            let projection = repo.projection(0, id)?;
            ensure!(projection.text == text, "original saved projection body");
            digest.bytes(&projection.encode()?);
            let proof = repo.full_l1_proof(0, id)?;
            digest.value(&proof)?;
            if let Some(ref proof) = proof {
                for reference in std::iter::once(Some(proof.original))
                    .chain([
                        proof.admission,
                        proof.final_auth,
                        proof.receiver,
                        proof.receiver_overlay,
                    ])
                    .flatten()
                {
                    let (raw, credential) = readers::credential(repo, reference)?;
                    ensure!(
                        scope.certs().contains(&credential.cert_ref()),
                        "original proof peer credential"
                    );
                    digest.bytes(&raw);
                }
            }
            let receipt = repo
                .receipt(0, role as u8, n)?
                .context("original Receipt1 missing")?;
            if role == 1 && n == 17 {
                ensure!(
                    blake3::hash(&receipt).to_hex().as_str()
                        == "338a74bcbee1249cb1655fa21962833d6d680394769861ca2df4ab98475a4760",
                    "original B17 Receipt1 pin"
                );
            }
            let decoded = old::decode(&receipt, 1024)?;
            let reference = decoded.fields(2)?[0].fields(8)?[7].fixed()?;
            let (receiver, _) = readers::credential(repo, reference)?;
            readers::verified_receipt(&scope, &verified, &receipt, &receiver)?;
            digest.bytes(&receipt);
            digest.bytes(&receiver);
            // A peer request index is neither read nor required. Only the local
            // author's immutable request ID points to this signed source.
            if role == expected.local {
                let local = repo.request(0, request)?;
                readers::local_request(expected.local, role, local.as_ref(), id, &text)?;
                digest.bytes(&request);
                digest.value(&local)?;
                own_requests += 1;
            }
            rows.insert(state::hex(&id), json!({"format":1,"id":state::hex(&id),
                "role":role,"ordinal":n,"sequence":e.core.sequence,"request":state::hex(&request),
                "text":text,"proof_kind":if proof.is_some(){"FullL1"}else{"LegacySnapshot"},"delivered":true}));
        }
    }
    Ok(Records {
        digest: digest.finish(),
        history: rows,
        sources: ids.len(),
        own_requests,
    })
}

pub(super) struct OriginalRecords {
    expected: fixed::Expected,
    anchor: Anchor,
    snapshot: Records,
}
impl OriginalRecords {
    pub(super) fn capture(node: &Node, local: usize) -> Result<Self> {
        ensure!(node.slot == 0, "G1 original occupied slot");
        let profile = Profile3::load(&node.profile)?;
        let mut repo = profile.open(&node.profile)?;
        let expected = fixed::Expected::production(local)?;
        let anchor = Anchor::capture(&mut repo, local)?;
        let snapshot = records(&mut repo, &expected, &anchor)?;
        ensure!(
            snapshot.history.len() == 528 && snapshot.sources == 530,
            "exact original Text/source count"
        );
        Ok(Self {
            expected,
            anchor,
            snapshot,
        })
    }
    pub(super) fn recheck(&self, node: &Node) -> Result<()> {
        let profile = Profile3::load(&node.profile)?;
        let mut repo = profile.open(&node.profile)?;
        ensure!(
            records(&mut repo, &self.expected, &self.anchor)? == self.snapshot,
            "original scope/source/AuthRecord/projection/Receipt1/own request changed"
        );
        Ok(())
    }
    pub(super) fn bind(&self, history: &OriginalHistory) -> Result<()> {
        exact_original(&history.rows, &self.snapshot.history)
    }
    pub(super) fn report(&self) -> Value {
        json!({"original_record_digest":self.snapshot.digest,"sources":self.snapshot.sources,
            "text_rows":self.snapshot.history.len(),"own_requests":self.snapshot.own_requests,
            "scope_bytes_blake3":blake3::hash(&self.anchor.raw).to_hex().to_string()})
    }
}

struct Pages {
    rows: Vec<Value>,
    pages: Vec<Value>,
    frontiers: [u64; 2],
    view: String,
}
fn pages(
    mut read: impl FnMut(usize, Option<&str>) -> Result<Value>,
    initial: Option<&str>,
    first_limit: usize,
    frozen: Option<([u64; 2], &str)>,
) -> Result<Pages> {
    let mut cursor = initial.map(str::to_owned);
    let mut cursors = BTreeSet::new();
    if let Some(ref c) = cursor {
        cursors.insert(c.clone());
    }
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    let mut saved = Vec::new();
    let mut stamp = frozen.map(|(f, v)| (f, v.to_owned()));
    let mut bytes = 0;
    for page_no in 0..34 {
        s53_deadline::check()?;
        let limit = if page_no == 0 { first_limit } else { 32 };
        let page = read(limit, cursor.as_deref())?;
        let size = serde_json::to_vec(&page)?.len();
        bytes += size;
        ensure!(
            size <= 48 * 1024 && bytes <= 4 * 1024 * 1024,
            "history page/total byte cap"
        );
        let f: [u64; 2] = serde_json::from_value(page["F"].clone())?;
        let view = page["view"].as_str().context("history view")?;
        ensure!(
            page["version"] == 3 && view.len() == 64 && view.bytes().all(|b| b.is_ascii_hexdigit()),
            "history version/view encoding"
        );
        if let Some((old_f, old_v)) = &stamp {
            ensure!(
                *old_f == f && old_v == view,
                "history frozen frontier/view changed"
            );
        } else {
            stamp = Some((f, view.into()));
        }
        let items = page["items"].as_array().context("history rows")?;
        ensure!(
            items.len() <= limit && rows.len() + items.len() <= 1024,
            "history row cap"
        );
        for row in items {
            let id = row["id"].as_str().context("history source ID")?;
            ensure!(ids.insert(id.to_owned()), "history duplicate source");
            rows.push(row.clone());
        }
        let encoded = page["cursor"].as_str().context("history cursor")?;
        kilogram_state::indexed_dialog::HistoryCursor::from_base64(encoded)?;
        let finished = page["finished"].as_bool().context("history finished")?;
        // A terminal request at an already exhausted cursor can return that
        // same cursor. Every nonempty page must advance its authenticated cursor.
        ensure!(
            (finished && items.is_empty()) || cursors.insert(encoded.into()),
            "history cursor did not advance"
        );
        saved.push(page.clone());
        if finished {
            let (frontiers, view) = stamp.context("history stamp")?;
            return Ok(Pages {
                rows,
                pages: saved,
                frontiers,
                view,
            });
        }
        ensure!(!items.is_empty(), "history empty unfinished page");
        cursor = Some(encoded.into());
    }
    anyhow::bail!("history page cap")
}
fn exact_original(rows: &[Value], expected: &BTreeMap<String, Value>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for row in rows {
        let id = row["id"].as_str().context("original history ID")?;
        ensure!(
            seen.insert(id) && expected.get(id) == Some(row),
            "original history source/projection/duplicate"
        );
    }
    ensure!(
        seen.len() == expected.len(),
        "original history truncated (including first row)"
    );
    Ok(())
}
pub(super) struct OriginalHistory {
    rows: Vec<Value>,
    pages: Vec<Value>,
    frontiers: [u64; 2],
    view: String,
}
impl OriginalHistory {
    fn read(
        read: impl FnMut(usize, Option<&str>) -> Result<Value>,
        frontiers: [u64; 2],
    ) -> Result<Self> {
        let p = pages(read, None, 1, None)?;
        ensure!(
            p.frontiers == frontiers
                && p.pages[0]["items"].as_array().is_some_and(|v| v.len() == 1),
            "original first page/frontiers"
        );
        Ok(Self {
            rows: p.rows,
            pages: p.pages,
            frontiers: p.frontiers,
            view: p.view,
        })
    }
    pub(super) fn capture(node: &Node) -> Result<Self> {
        let history = Self::read(|n, c| node.history(n, c), [512, 18])?;
        ensure!(
            history.rows.len() == 528,
            "528 original Text IDs including first page"
        );
        Ok(history)
    }
    fn compare(&self, mut read: impl FnMut(usize, Option<&str>) -> Result<Value>) -> Result<()> {
        // Fresh pagination is a new frontier and may have another causal order.
        // Compare only the complete original logical projection, keyed by ID.
        let fresh = pages(&mut read, None, 1, None)?;
        ensure!(
            fresh
                .frontiers
                .iter()
                .zip(self.frontiers)
                .all(|(now, old)| *now >= old),
            "history frontier rollback"
        );
        let expected: BTreeMap<_, _> = self
            .rows
            .iter()
            .map(|r| {
                Ok((
                    r["id"].as_str().context("saved original ID")?.to_owned(),
                    r.clone(),
                ))
            })
            .collect::<Result<_>>()?;
        let originals: Vec<_> = fresh
            .rows
            .into_iter()
            .filter(|r| expected.contains_key(r["id"].as_str().unwrap_or("")) || r["format"] == 1)
            .collect();
        exact_original(&originals, &expected)?;
        let cursor = self.pages[0]["cursor"]
            .as_str()
            .context("original first cursor")?;
        let frozen = pages(
            &mut read,
            Some(cursor),
            32,
            Some((self.frontiers, &self.view)),
        )?;
        ensure!(
            frozen.rows == self.rows[1..],
            "original frozen suffix changed"
        );
        let last = self.pages.last().context("original terminal page")?["cursor"]
            .as_str()
            .context("original terminal cursor")?;
        let terminal = pages(read, Some(last), 32, Some((self.frontiers, &self.view)))?;
        ensure!(
            terminal.rows.is_empty(),
            "original terminal cursor advanced past frozen frontier"
        );
        Ok(())
    }
    pub(super) fn recheck(&self, node: &Node) -> Result<()> {
        self.compare(|n, c| node.history(n, c))
    }
    pub(super) fn report(&self) -> Result<Value> {
        Ok(
            json!({"text_rows":self.rows.len(),"pages":self.pages.len(),"frozen_suffix_rows":self.rows.len()-1,
            "frontiers":self.frontiers,"view":self.view,
            "first_page_blake3":blake3::hash(&serde_json::to_vec(&self.pages[0])?).to_hex().to_string(),
            "all_original_pages_blake3":blake3::hash(&serde_json::to_vec(&self.pages)?).to_hex().to_string()}),
        )
    }
}

#[test]
fn g1b1_genuine_original_records_and_receipt_negatives() -> Result<()> {
    let mut fixture = fixture::small()?;
    for local in 0..2 {
        let repo = &mut fixture.repositories[local];
        let expected = &fixture.expected[local];
        let anchor = Anchor::capture(repo, local)?;
        let first = records(repo, expected, &anchor)?;
        ensure!(
            first.sources == 4 && first.history.len() == 2 && first.own_requests == 1,
            "genuine small record counts"
        );
        ensure!(
            records(repo, expected, &anchor)? == first,
            "genuine immutable record repeat"
        );
        let scope = anchor.verify(repo)?;
        let (request, text) = expected.text(local, 1);
        let raw = repo.event_at(0, local as u8, 1)?.context("own source")?;
        let parsed = old::Event::parse(&raw, scope.peers()[local].certificate().device_id())?;
        let (auth, _) = readers::credential(repo, parsed.core.author_auth)?;
        let source = readers::verified_source(&scope, &raw, &auth, local, 1)?;
        let id = source.event().id()?;
        let own = repo.request(0, request)?;
        readers::local_request(local, local, own.as_ref(), id, &text)?;
        let missing = readers::local_request(local, local, None, id, &text)
            .err()
            .context("missing own request accepted")?;
        ensure!(
            missing.to_string() == "local request required",
            "exact own absence premise"
        );
        let peer = 1 - local;
        let (peer_request, peer_text) = expected.text(peer, 1);
        ensure!(
            repo.request(0, peer_request)?.is_none(),
            "genuine peer request absent"
        );
        readers::local_request(local, peer, None, id, &peer_text)?;
        let receipt = repo.receipt(0, local as u8, 1)?.context("own Receipt1")?;
        let value = old::decode(&receipt, 1024)?;
        let reference = value.fields(2)?[0].fields(8)?[7].fixed()?;
        let (receiver, _) = readers::credential(repo, reference)?;
        readers::verified_receipt(&scope, &source, &receipt, &receiver)?;
        let mut bad = receipt.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        wire::ParsedReceipt1::parse(&bad)?;
        ensure!(
            readers::verified_receipt(&scope, &source, &bad, &receiver).is_err(),
            "Receipt1 signature corruption accepted"
        );
        let mut fields = value.fields(2)?.to_vec();
        let mut body = fields[0].fields(8)?.to_vec();
        body[5] = old::b([203; 32]);
        fields[0] = old::a(body);
        let bad_binding = old::encode(&old::a(fields), 1024)?;
        wire::ParsedReceipt1::parse(&bad_binding)?;
        let error = readers::verified_receipt(&scope, &source, &bad_binding, &receiver)
            .err()
            .context("wrong source Receipt1 accepted")?;
        ensure!(
            error.to_string().contains("pre-Q Receipt1 source/receiver"),
            "exact Receipt1 source correspondence premise"
        );
        ensure!(
            readers::verified_receipt(&scope, &source, &receipt, &auth).is_err(),
            "wrong receiver accepted"
        );
        println!(
            "g1b1_records local={local} sources=4 texts=2 own_requests=1 peer_absent_valid=true own_absent_refused=true receipt_signature_refused=true receipt_source_refused=true receiver_refused=true"
        );
    }
    Ok(())
}

#[test]
fn g1b1_genuine_history_first_row_suffix_and_negatives() -> Result<()> {
    let mut fixture = fixture::small()?;
    for local in 0..2 {
        let repo = &mut fixture.repositories[local];
        let expected = &fixture.expected[local];
        let anchor = Anchor::capture(repo, local)?;
        let signed = records(repo, expected, &anchor)?;
        let mut read = |n, c: Option<&str>| -> Result<Value> {
            let cursor = c
                .map(kilogram_state::indexed_dialog::HistoryCursor::from_base64)
                .transpose()?;
            Ok(serde_json::from_slice(&repo.history(
                0,
                cursor.as_ref(),
                n,
            )?)?)
        };
        let capture = OriginalHistory::read(&mut read, [2, 2])?;
        exact_original(&capture.rows, &signed.history)?;
        capture.compare(&mut read)?;
        ensure!(
            capture.rows.len() == 2 && capture.pages.len() == 2,
            "first page plus genuine suffix"
        );
        let omitted = &capture.rows[1..];
        let error = exact_original(omitted, &signed.history)
            .err()
            .context("first row omitted accepted")?;
        ensure!(
            error.to_string().contains("including first row"),
            "first omission exact premise"
        );
        ensure!(
            exact_original(&capture.rows[..1], &signed.history).is_err(),
            "truncated original accepted"
        );
        let duplicate = vec![capture.rows[0].clone(), capture.rows[0].clone()];
        ensure!(
            exact_original(&duplicate, &signed.history).is_err(),
            "duplicate original accepted"
        );
        let mut damaged = capture.rows.clone();
        damaged[0]["text"] = json!("changed body");
        ensure!(
            exact_original(&damaged, &signed.history).is_err(),
            "changed first projection accepted"
        );
        // Exercise the actual pagination path with corrupted genuine responses,
        // leaving the database and its authenticated cursors untouched.
        let error = capture
            .compare(|n, c| {
                let mut page = read(n, c)?;
                if c.is_none() {
                    page["items"] = json!([]);
                }
                Ok(page)
            })
            .err()
            .context("omitted public first page accepted")?;
        ensure!(
            error.to_string() == "history empty unfinished page",
            "first page progress premise"
        );
        // Fresh ordering is not the frozen ordering oracle. These are genuine
        // verified rows; only a new-view response envelope is simulated here.
        let mut reordered = capture.rows.clone();
        reordered.reverse();
        exact_original(&reordered, &signed.history)?;
        println!(
            "g1b1_history local={local} genuine_first=1 suffix=1 complete=2 frozen_replay=true first_omission_refused=true duplicate_refused=true truncation_refused=true projection_refused=true fresh_reorder_allowed=true"
        );
    }
    Ok(())
}
