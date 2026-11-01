//! Original-only Cold-B anchor replay. Borrowed repository; no owner or mutation.
use anyhow::{Context, Result, ensure};
use kilogram_cli::ordered_dialog::{
    handshake::Spec,
    runtime7::Profile3,
    state::{self, Head},
};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, HistoryCursor, migration7::live1 as store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

// Reuse the accepted pure verifiers/fixture without editing their source.
#[path = "s53_cold/fixed.rs"]
mod fixed;
#[cfg(test)]
#[path = "s53_cold/fixture.rs"]
mod fixture;
#[path = "s53_cold/history.rs"]
mod history;
#[path = "s53_cold/progress.rs"]
mod progress;
#[path = "s53_cold/readers.rs"]
mod readers;
#[cfg(test)]
#[path = "migration_s53_g1_replay_tests.rs"]
mod tests;

const PAGE_CAP: usize = 48 * 1024 - 64;
const TOTAL_CAP: usize = 4 * 1024 * 1024;
const EVIDENCE: &str = "ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence";
const RUN: &str = ".tmp/migration-s53-integrated/F-s53-exhausted-integrated-01-attempt-01";
const B1: [&str; 2] = [
    "955e68c7d18222f910d1411aa389556eba25a240b23f2304301cd1a72fc90ef9",
    "5b5e77ae80e850f1e5310a141e25f629cef0bf6047d54751f6055713b1cf2de7",
];
const LOGICAL: [&str; 2] = [
    "749847c5d0555002adb8291e200c3a5a8e2e0a2d882411820304cff4430ac63e",
    "6f10324a9708c4ed5eeaedcb638a6fefbc0fe54970851a5a575553631cac08d6",
];

fn check(until: Instant) -> Result<()> {
    super::s53_deadline::check()?;
    readers::check(until)
}
fn hex(s: &str, bytes: usize) -> bool {
    s.len() == bytes * 2
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn sha(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    path: String,
    bytes: usize,
    sha256: String,
}
fn pinned(path: &Path, pin: &Pin, cap: usize) -> Result<Vec<u8>> {
    ensure!(
        pin.bytes > 0 && pin.bytes <= cap && hex(&pin.sha256, 32),
        "C1 public pin shape"
    );
    let raw = kilogram_cli::ordered_dialog::admin::read_public(path, cap)?;
    ensure!(
        raw.len() == pin.bytes && sha(&raw) == pin.sha256,
        "C1 public pin mismatch"
    );
    Ok(raw)
}

// Field order deliberately matches live1/history.rs. Value's sorted map cannot
// reproduce the raw repository bytes which Cold-B hashes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    format: u8,
    id: String,
    role: u8,
    ordinal: u64,
    sequence: u64,
    request: String,
    text: String,
    proof_kind: String,
    delivered: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    version: u8,
    view: String,
    #[serde(rename = "F")]
    frontiers: [u64; 2],
    finished: bool,
    cursor: String,
    items: Vec<Row>,
}
impl Page {
    fn parse(raw: &[u8]) -> Result<Self> {
        ensure!(
            !raw.is_empty() && raw.len() <= 48 * 1024,
            "C1 public page byte cap"
        );
        Ok(serde_json::from_slice(raw)?)
    }
    fn encode(&self) -> Result<Vec<u8>> {
        let raw = serde_json::to_vec(self)?;
        ensure!(raw.len() <= PAGE_CAP, "C1 combined page encoded byte cap");
        Ok(raw)
    }
    fn envelope(
        &self,
        scope: &wire::VerifiedExistingScope1,
        f: [u64; 2],
        view: &str,
        next: [u64; 2],
    ) -> Result<()> {
        ensure!(
            self.version == 3 && self.frontiers == f && self.view == view && hex(view, 32),
            "C1 original page frontier/view"
        );
        let cursor = HistoryCursor::from_base64(&self.cursor)?;
        let b = cursor.as_bytes();
        // These are only public transport-shape checks. Repository.history must
        // authenticate the MAC/identity before any replay can succeed.
        ensure!(
            state::hex(&b[40..72]) == view
                && b[72..104] == scope.c()
                && b[104..136] == scope.d()
                && b[136..168] == scope.e()
                && b[168..211] == *scope.sid().as_bytes(),
            "C1 cursor public domain"
        );
        let numbers: Vec<_> = b[211..243]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|n| u64::from_be_bytes(*n))
            .collect();
        ensure!(
            numbers == [f[0], f[1], next[0], next[1]],
            "C1 cursor frontier/position"
        );
        ensure!(self.finished == (next == f), "C1 page terminal position");
        Ok(())
    }
}
fn joined(first: &Row, tail: &Page) -> Result<Vec<u8>> {
    ensure!(tail.items.len() < 32, "C1 first-page join count");
    let mut page = tail.clone();
    page.items.insert(0, first.clone());
    page.encode()
}

pub(super) struct Original {
    expected: fixed::Expected,
    origin: Vec<u8>,
    authorities: [Vec<u8>; 2],
    pages: Vec<Page>,
    ordered: Vec<Row>,
    map: BTreeMap<String, Value>,
    b1: String,
    logical: String,
}
#[derive(Debug, Serialize)]
pub(super) struct Report {
    pub(super) b1: String,
    pub(super) logical: String,
    pub(super) sources: usize,
    pub(super) texts: usize,
    pub(super) own_requests: usize,
    pub(super) reconstructed_pages: usize,
    pub(super) frozen_suffix_rows: usize,
}

/// Opens only the fixed hash-bound PUBLIC attempt01 bundle. No private paths.
pub(super) fn public_originals() -> Result<[Original; 2]> {
    let workspace = Path::new(fixed::WORKSPACE);
    let manifest = pinned(
        &workspace
            .join(EVIDENCE)
            .join("F-s53-exhausted-integrated-01-attempt-01.public-command-files.json"),
        &Pin {
            path: String::new(),
            bytes: 509647,
            sha256: "aea46b7e73c8c44163945ae576fd0ae8079498a1fcae38c16e18288e137c24c3".into(),
        },
        1024 * 1024,
    )?;
    let value: Value = serde_json::from_slice(&manifest)?;
    let pins: Vec<Pin> = serde_json::from_value(value["files"].clone())?;
    ensure!(pins.len() == 2243, "C1 public inventory cardinality");
    let read = |name: &str, cap| -> Result<Vec<u8>> {
        // name is generated below, never obtained from an untrusted path field.
        let relative = format!("{RUN}/{name}");
        let rows: Vec<_> = pins.iter().filter(|p| p.path == relative).collect();
        ensure!(rows.len() == 1, "C1 exactly one public input pin");
        pinned(&workspace.join(&relative), rows[0], cap)
    };
    let origin = read("exhausted.origin.cbor", wire::MAX_EXISTING_ORIGIN)?;
    let authorities = [
        read("exhausted.a.authority", wire::MAX_AUTH)?,
        read("exhausted.b.authority", wire::MAX_AUTH)?,
    ];
    let mut result = Vec::new();
    for local in 0..2 {
        let pages = (2 + local * 18..20 + local * 18)
            .map(|n| read(&format!("command-{n:04}.stdout.log"), 48 * 1024))
            .collect::<Result<Vec<_>>>()?;
        result.push(Original::new(
            fixed::Expected::production(local)?,
            origin.clone(),
            authorities.clone(),
            &pages,
            B1[local].into(),
            LOGICAL[local].into(),
        )?);
    }
    result
        .try_into()
        .map_err(|_| anyhow::anyhow!("C1 original pair"))
}

impl Original {
    pub(super) fn origin(&self) -> &[u8] {
        &self.origin
    }
    pub(super) fn compare_original_map(&self, current: &BTreeMap<String, Value>) -> Result<()> {
        let originals: BTreeMap<_, _> = current
            .iter()
            .filter(|(id, row)| self.map.contains_key(*id) || row["format"] == 1)
            .map(|(id, row)| (id.clone(), row.clone()))
            .collect();
        ensure!(
            originals == self.map,
            "C2 complete pinned original history map"
        );
        Ok(())
    }
    fn new(
        expected: fixed::Expected,
        origin: Vec<u8>,
        authorities: [Vec<u8>; 2],
        raw_pages: &[Vec<u8>],
        b1: String,
        logical: String,
    ) -> Result<Self> {
        ensure!(
            expected.local < 2
                && expected.next.iter().all(|n| (2..=512).contains(n))
                && hex(&b1, 32)
                && hex(&logical, 32),
            "C1 bounded original plan"
        );
        let scope = wire::VerifiedExistingScope1::verify_export(&origin)?;
        expected.scope(&scope)?;
        for (role, authority) in authorities.iter().enumerate() {
            readers::authority(
                authority,
                &scope.peers()[role].authority().encode()?,
                scope.peers()[role].certificate().account_id(),
            )?;
        }
        let total = usize::try_from(expected.next.iter().sum::<u64>() - 2)?;
        ensure!(
            raw_pages.len() == 1 + (total - 1).div_ceil(32)
                && raw_pages.len() <= 34
                && raw_pages.iter().map(Vec::len).sum::<usize>() <= TOTAL_CAP,
            "C1 complete original page count/bytes"
        );
        let pages = raw_pages
            .iter()
            .map(|raw| Page::parse(raw))
            .collect::<Result<Vec<_>>>()?;
        let mut ordered = Vec::new();
        let mut map = BTreeMap::new();
        let mut positions = BTreeSet::new();
        let mut cursors = BTreeSet::new();
        let mut next = [1, 1];
        for (i, page) in pages.iter().enumerate() {
            let count = if i == 0 {
                1
            } else {
                (total - ordered.len()).min(32)
            };
            ensure!(
                page.items.len() == count && count > 0,
                "C1 complete original page rows"
            );
            for row in &page.items {
                ensure!(
                    row.format == 1
                        && row.role < 2
                        && row.ordinal > 0
                        && row.ordinal < expected.next[usize::from(row.role)]
                        && hex(&row.id, 32)
                        && hex(&row.request, 16)
                        && row.delivered
                        && matches!(row.proof_kind.as_str(), "LegacySnapshot" | "FullL1"),
                    "C1 original row shape"
                );
                ensure!(
                    row.ordinal == next[usize::from(row.role)]
                        && positions.insert((row.role, row.ordinal)),
                    "C1 original ordered lane/ordinal"
                );
                next[usize::from(row.role)] += 1;
                ensure!(
                    map.insert(row.id.clone(), serde_json::to_value(row)?)
                        .is_none(),
                    "C1 duplicate original ID"
                );
                ordered.push(row.clone());
            }
            page.envelope(&scope, expected.next, &pages[0].view, next)?;
            ensure!(
                cursors.insert(page.cursor.clone()) && page.finished == (i + 1 == pages.len()),
                "C1 original cursor/terminal progress"
            );
        }
        ensure!(
            ordered.len() == total && next == expected.next,
            "C1 complete original map"
        );
        let mut hd = readers::Digest::new(b"kg:s53:cold:history:1\0");
        hd.value(&map)?;
        ensure!(hd.finish() == logical, "C1 pinned original logical digest");
        Ok(Self {
            expected,
            origin,
            authorities,
            pages,
            ordered,
            map,
            b1,
            logical,
        })
    }

    fn scope(&self, repo: &mut store::L1Repository) -> Result<wire::VerifiedExistingScope1> {
        let origin = old::decode(&self.origin, wire::MAX_EXISTING_ORIGIN)?;
        let f = origin.fields(7)?;
        ensure!(
            f[1].bytes()? == repo.spec(0)?
                && f[4].bytes()?
                    == repo
                        .control(0, store::L1Control::InitialPair)?
                        .context("C1 original pair")?,
            "C1 original stored scope"
        );
        for part in [2, 3] {
            let raw = f[part].bytes()?;
            ensure!(
                repo.signed_control(0, old::hash(old::CONTROL_ID, raw))? == raw,
                "C1 original Offer/Grant bytes"
            );
        }
        for role in 0..2 {
            ensure!(
                f[5].fields(2)?[role].bytes()?
                    == repo.event_at(0, role as u8, 0)?.context("C1 genesis")?
                    && f[6].fields(2)?[role].bytes()?
                        == repo
                            .control(0, store::L1Control::Ready(role as u8))?
                            .context("C1 Ready")?,
                "C1 original genesis/Ready bytes"
            );
        }
        let scope = wire::VerifiedExistingScope1::verify_export(&self.origin)?;
        self.expected.scope(&scope)?;
        for (role, proof) in self.authorities.iter().enumerate() {
            let account = scope.peers()[role].certificate().account_id();
            readers::authority(proof, &repo.authority(*account.as_bytes())?, account)?;
        }
        Ok(scope)
    }

    fn records(
        &self,
        repo: &mut store::L1Repository,
        scope: &wire::VerifiedExistingScope1,
        until: Instant,
        digest: &mut readers::Digest,
    ) -> Result<usize> {
        digest.bytes(&self.origin);
        let mut ids = BTreeSet::new();
        let mut map = BTreeMap::new();
        let mut own_requests = 0;
        for role in 0..2 {
            let mut previous = None;
            for n in 0..self.expected.next[role] {
                check(until)?;
                let raw = repo
                    .event_at(0, role as u8, n)?
                    .context("C1 original source")?;
                let parsed =
                    old::Event::parse(&raw, scope.peers()[role].certificate().device_id())?;
                let (auth, author) = readers::credential(repo, parsed.core.author_auth)?;
                readers::authority(
                    &self.authorities[role],
                    &author.authority().encode()?,
                    author.certificate().account_id(),
                )?;
                let verified = readers::verified_source(scope, &raw, &auth, role, n)?;
                let e = verified.event();
                let id = e.id()?;
                ensure!(
                    ids.insert(id) && e.core.previous == previous,
                    "C1 original chain/uniqueness"
                );
                previous = Some(id);
                digest.bytes(&raw);
                digest.bytes(&auth);
                if n == 0 {
                    continue;
                }
                let (request, text) = self.expected.text(role, n);
                ensure!(
                    e.core.request == request && !fixed::future_ids().contains(&request),
                    "C1 original signed request"
                );
                let projection = repo.projection(0, id)?;
                ensure!(projection.text == text, "C1 original projection body");
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
                        let r = scope
                            .certs()
                            .iter()
                            .position(|c| *c == credential.cert_ref())
                            .context("C1 original proof peer")?;
                        readers::authority(
                            &self.authorities[r],
                            &credential.authority().encode()?,
                            credential.certificate().account_id(),
                        )?;
                        digest.bytes(&raw);
                    }
                }
                let receipt = repo
                    .receipt(0, role as u8, n)?
                    .context("C1 original Receipt1")?;
                if role == 1 && n == 17 {
                    ensure!(
                        blake3::hash(&receipt).to_hex().as_str()
                            == "338a74bcbee1249cb1655fa21962833d6d680394769861ca2df4ab98475a4760",
                        "C1 B17 Receipt1 pin"
                    );
                }
                let parsed = old::decode(&receipt, 1024)?;
                let reference = parsed.fields(2)?[0].fields(8)?[7].fixed()?;
                let (receiver, credential) = readers::credential(repo, reference)?;
                ensure!(
                    credential.cert_ref() == scope.certs()[1 - role],
                    "C1 opposite receipt signer"
                );
                readers::authority(
                    &self.authorities[1 - role],
                    &credential.authority().encode()?,
                    credential.certificate().account_id(),
                )?;
                readers::verified_receipt(scope, &verified, &receipt, &receiver)?;
                digest.bytes(&receipt);
                digest.bytes(&receiver);
                if role == self.expected.local {
                    let own = repo.request(0, request)?;
                    readers::local_request(self.expected.local, role, own.as_ref(), id, &text)?;
                    // Exact Cold-B optional serialization, without G1's extra ID bytes.
                    digest.value(&own)?;
                    own_requests += 1;
                }
                map.insert(state::hex(&id), json!({"format":1,"id":state::hex(&id),"role":role,"ordinal":n,"sequence":e.core.sequence,"request":state::hex(&request),"text":text,"proof_kind":if proof.is_some(){"FullL1"}else{"LegacySnapshot"},"delivered":true}));
            }
        }
        ensure!(
            map == self.map,
            "C1 all signed originals equal pinned history map"
        );
        Ok(own_requests)
    }

    fn history_bytes(
        &self,
        mut read: impl FnMut(usize, &str) -> Result<Vec<u8>>,
        until: Instant,
    ) -> Result<Vec<Vec<u8>>> {
        let scope = wire::VerifiedExistingScope1::verify_export(&self.origin)?;
        let total = self.ordered.len();
        let first_count = total.min(32);
        check(until)?;
        // Production requires exactly31 additional rows; tiny genuine fixtures
        // may finish earlier, but cannot change the fixed production total528.
        let tail = Page::parse(&read(31, &self.pages[0].cursor)?)?;
        ensure!(
            tail.items == self.ordered[1..first_count],
            "C1 exact first31-row join"
        );
        let mut next = [1, 1];
        for row in &self.ordered[..first_count] {
            next[usize::from(row.role)] += 1;
        }
        tail.envelope(&scope, self.expected.next, &self.pages[0].view, next)?;
        let mut result = vec![joined(&self.ordered[0], &tail)?];
        let mut current = tail;
        let mut cursor_set = BTreeSet::from([self.pages[0].cursor.clone(), current.cursor.clone()]);
        let mut count = first_count;
        let mut bytes = result[0].len();
        while count < total {
            check(until)?;
            ensure!(
                result.len() < 32 && !current.finished,
                "C1 replay page progress"
            );
            let raw = read(32, &current.cursor)?;
            ensure!(raw.len() <= PAGE_CAP, "C1 replay page byte cap");
            current = Page::parse(&raw)?;
            let end = (count + 32).min(total);
            ensure!(
                current.items == self.ordered[count..end],
                "C1 exact replay page rows"
            );
            for row in &current.items {
                next[usize::from(row.role)] += 1;
            }
            current.envelope(&scope, self.expected.next, &self.pages[0].view, next)?;
            ensure!(
                cursor_set.insert(current.cursor.clone()),
                "C1 replay cursor progress"
            );
            // Also prove that subsequent raw pages use the same exact serializer.
            ensure!(current.encode()? == raw, "C1 raw page serializer mismatch");
            bytes = bytes
                .checked_add(raw.len())
                .context("C1 history byte overflow")?;
            ensure!(bytes <= TOTAL_CAP, "C1 total history byte cap");
            result.push(raw);
            count = end;
        }
        ensure!(
            current.finished && next == self.expected.next,
            "C1 replay terminal frontier"
        );
        // Independent original first1+suffix32 partition, not the reconstructed
        // partition. Every supplied old cursor is authenticated by the reader.
        let mut cursor = &self.pages[0].cursor;
        for saved in &self.pages[1..] {
            check(until)?;
            let now = Page::parse(&read(32, cursor)?)?;
            ensure!(now == *saved, "C1 pinned frozen suffix changed");
            cursor = &saved.cursor;
        }
        check(until)?;
        let terminal = Page::parse(&read(32, cursor)?)?;
        terminal.envelope(
            &scope,
            self.expected.next,
            &self.pages[0].view,
            self.expected.next,
        )?;
        ensure!(
            terminal.items.is_empty() && terminal.cursor == *cursor,
            "C1 old terminal cursor advanced"
        );
        Ok(result)
    }

    pub(super) fn replay(&self, repo: &mut store::L1Repository, until: Instant) -> Result<Report> {
        check(until)?;
        let scope = self.scope(repo)?;
        let mut digest = readers::Digest::new(b"kg:s53:cold:B1:1\0");
        let own_requests = self.records(repo, &scope, until, &mut digest)?;
        let pages = self.history_bytes(
            |n, cursor| {
                repo.history(0, Some(&HistoryCursor::from_base64(cursor)?), n)
                    .map_err(Into::into)
            },
            until,
        )?;
        for page in &pages {
            digest.bytes(page);
        }
        let b1 = digest.finish();
        ensure!(b1 == self.b1, "C1 immutable Cold-B B1 digest mismatch");
        let mut hd = readers::Digest::new(b"kg:s53:cold:history:1\0");
        hd.value(&self.map)?;
        let logical = hd.finish();
        ensure!(
            logical == self.logical,
            "C1 immutable logical digest mismatch"
        );
        Ok(Report {
            b1,
            logical,
            sources: self.map.len() + 2,
            texts: self.map.len(),
            own_requests,
            reconstructed_pages: pages.len(),
            frozen_suffix_rows: self.ordered.len() - 1,
        })
    }
}
