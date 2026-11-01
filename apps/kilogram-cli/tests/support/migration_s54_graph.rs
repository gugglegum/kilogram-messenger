//! Bounded supplied authenticated Text inventory, not a repository/Native oracle.
use super::*;
use anyhow::Context as _;
use kilogram_protocol::ordered_dialog_v2 as v2;
#[cfg(test)]
#[path = "migration_s54_graph_tests.rs"]
mod tests;
const TEXTS: usize = 1344;
fn limit(context: usize) -> u16 {
    if context == 0 { 640 } else { 32 }
}
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Scope {
    context: old::Id,
    d: old::Id,
    e: old::Id,
    sid: String,
    selected: old::Id,
    certs: [old::Id; 2],
    controls: [old::Id; 4],
    origin: old::Id,
}
impl Scope {
    fn genesis(&self, role: usize) -> old::Id {
        self.controls[role + 2]
    }
    fn commitment(&self) -> Result<old::Id> {
        Ok(digest(&old::encode(
            &old::a([
                old::u(1),
                old::b(self.context),
                old::b(self.d),
                old::b(self.e),
                old::b(self.sid.as_bytes()),
                old::b(self.selected),
                old::a(self.certs.iter().map(old::b)),
                old::a(self.controls.iter().map(old::b)),
                old::b(self.origin),
            ]),
            2048,
        )?))
    }
}
fn scopes_valid(scopes: &[Scope; 2]) -> Result<()> {
    ensure!(
        scopes[0].context != scopes[1].context
            && scopes.iter().all(|s| s.sid.len() == 43 && s.sid.is_ascii()),
        "distinct original bounded context scopes"
    );
    let anchors = scopes
        .iter()
        .flat_map(|s| [s.genesis(0), s.genesis(1)])
        .collect::<BTreeSet<_>>();
    ensure!(
        anchors.len() == 4,
        "four distinct authenticated genesis anchors"
    );
    Ok(())
}
pub(super) struct Graph {
    scopes: [Scope; 2],
    rows: BTreeMap<(usize, u8, u16), records::Record>,
    ids: BTreeMap<old::Id, (usize, u8, u16)>,
    requests: BTreeSet<[u8; 16]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Context {
    pub context: old::Id,
    pub scope_commitment: old::Id,
    pub genesis: [old::Id; 2],
    pub count: usize,
    pub next: [u64; 2],
    pub tails: [old::Id; 2],
    pub ordered: Vec<old::Id>,
    pub records_commitment: old::Id,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Terminal {
    pub contexts: [Context; 2],
    pub texts: usize,
    pub genesis: usize,
}
impl Terminal {
    pub fn same(&self, other: &Self) -> Result<()> {
        ensure!(
            self == other,
            "exact supplied owner/cut graph including scopes, order and all normalized commitments"
        );
        Ok(())
    }
    pub fn stored(&self, context: usize, next: [u64; 2], tails: [old::Id; 2]) -> Result<()> {
        let actual = self
            .contexts
            .get(context)
            .context("bounded original stored context")?;
        ensure!(
            actual.next == next && actual.tails == tails,
            "stored terminal next/tails equal derived actual graph"
        );
        Ok(())
    }
}
fn record_commitment(r: &records::Record) -> Result<old::Id> {
    let fields = old::a([
        old::u(1),
        old::u(u64::from(r.format)),
        old::b(r.event),
        old::b(r.context),
        old::b(r.d),
        old::b(r.e),
        old::b(r.sid.as_bytes()),
        old::b(r.selection),
        old::u(u64::from(r.role)),
        old::u(r.ordinal),
        old::b(r.request),
        old::opt(r.previous.map(old::b)),
        old::a(r.parents.iter().map(old::b)),
        old::u(r.sequence),
        old::b(r.author_auth),
        old::b(r.source_hash),
        old::b(r.ciphertext_hash),
        old::b(r.body_hash),
        old::u(u64::from(r.receipt_format)),
        old::b(r.receipt_hash),
        old::b(r.receiver_auth),
    ]);
    Ok(old::hash(
        b"kg:s54:terminal-record:v1\0",
        &old::encode(&fields, 2048)?,
    ))
}
impl Graph {
    pub fn new(
        origins: [&v2::VerifiedExistingScope1; 2],
        proofs: [v2::ParsedModeProof2; 2],
    ) -> Result<Self> {
        s53_deadline::check()?;
        let mut scopes = Vec::new();
        for (index, (origin, proof)) in origins.into_iter().zip(proofs).enumerate() {
            s53_deadline::check()?;
            let selected = origin.selection(proof)?;
            ensure!(
                selected.proof().proof().cuts() == if index == 0 { [25, 25] } else { [33, 33] },
                "original C1/C2 selected cuts"
            );
            scopes.push(Scope {
                context: origin.c(),
                d: origin.d(),
                e: origin.e(),
                sid: origin.sid().into(),
                selected: selected.proof().q(),
                certs: origin.certs(),
                controls: origin.old_control_ids(),
                origin: digest(origin.raw()),
            });
        }
        let scopes = scopes
            .try_into()
            .map_err(|_| anyhow::anyhow!("two original authenticated scopes"))?;
        scopes_valid(&scopes)?;
        Ok(Self {
            scopes,
            rows: BTreeMap::new(),
            ids: BTreeMap::new(),
            requests: BTreeSet::new(),
        })
    }
    #[cfg(test)]
    fn synthetic(scopes: [Scope; 2]) -> Result<Self> {
        scopes_valid(&scopes)?;
        Ok(Self {
            scopes,
            rows: BTreeMap::new(),
            ids: BTreeMap::new(),
            requests: BTreeSet::new(),
        })
    }
    pub fn insert(&mut self, row: records::Record) -> Result<()> {
        s53_deadline::check()?;
        ensure!(self.rows.len() < TEXTS, "bounded1344 supplied Text records");
        let context = self
            .scopes
            .iter()
            .position(|s| s.context == row.context)
            .context("original record context")?;
        let scope = &self.scopes[context];
        ensure!(
            row.role < 2 && (1..=u64::from(limit(context))).contains(&row.ordinal),
            "bounded original Text lane/ordinal"
        );
        let n = u16::try_from(row.ordinal)?;
        let key = (context, row.role, n);
        let request = oracle::Request::new(context as u8, row.role, n)?;
        let format = if context == 0 && n >= 25 { 2 } else { 1 };
        ensure!(
            row.d == scope.d
                && row.e == scope.e
                && row.sid == scope.sid
                && row.selection == scope.selected
                && row.format == format
                && matches!(row.receipt_format, 1 | 2)
                && (format == 1 || row.receipt_format == 2),
            "same supplied authenticated scope/selection/source and independent receipt encoding"
        );
        ensure!(
            row.request == source::parse_request_id(&source::request_id(request))?
                && row.previous.is_some()
                && row.parents.len() <= 2
                && row.parents.windows(2).all(|w| w[0] < w[1]),
            "original request and canonical bounded signed graph fields"
        );
        ensure!(
            !self.rows.contains_key(&key)
                && !self.ids.contains_key(&row.event)
                && !self.requests.contains(&row.request)
                && self
                    .scopes
                    .iter()
                    .all(|s| row.event != s.genesis(0) && row.event != s.genesis(1)),
            "unique Text position/EventID/request separate from all genesis"
        );
        self.requests.insert(row.request);
        self.ids.insert(row.event, key);
        self.rows.insert(key, row);
        Ok(())
    }
    pub fn finish(self) -> Result<Terminal> {
        s53_deadline::check()?;
        ensure!(
            self.rows.len() == TEXTS && self.ids.len() == TEXTS && self.requests.len() == TEXTS,
            "complete exact supplied Text set, no prefix"
        );
        let mut contexts = Vec::new();
        for (context, scope) in self.scopes.iter().enumerate() {
            let mut hash = blake3::Hasher::new();
            hash.update(b"kg:s54:terminal-inventory:v1\0");
            hash.update(&scope.commitment()?);
            for role in 0..2u8 {
                let mut previous = scope.genesis(role as usize);
                for n in 1..=limit(context) {
                    s53_deadline::check()?;
                    let row = self
                        .rows
                        .get(&(context, role, n))
                        .context("complete contiguous original lane")?;
                    ensure!(
                        row.previous == Some(previous),
                        "exact authenticated genesis/previous lane source"
                    );
                    for parent in &row.parents {
                        let position = self.ids.get(parent).context(
                            "parent must be an actual supplied Text, not missing/genesis",
                        )?;
                        ensure!(
                            position.0 == context && *parent != row.event,
                            "same-context non-self parent mapping"
                        );
                    }
                    hash.update(&record_commitment(row)?);
                    previous = row.event;
                }
            }
            let mut next = [1u64; 2];
            let mut tails = [scope.genesis(0), scope.genesis(1)];
            let mut ordered = Vec::with_capacity(usize::from(limit(context)) * 2);
            while ordered.len() < usize::from(limit(context)) * 2 {
                s53_deadline::check()?;
                let candidate = (0..2u8)
                    .filter_map(|role| {
                        self.rows
                            .get(&(context, role, u16::try_from(next[role as usize]).ok()?))
                    })
                    .filter(|row| {
                        row.parents.iter().all(|p| {
                            self.ids.get(p).is_some_and(|position| {
                                u64::from(position.2) < next[position.1 as usize]
                            })
                        })
                    })
                    .min_by_key(|row| row.event)
                    .context("history causal frontier blocked/cyclic supplied graph")?;
                next[candidate.role as usize] += 1;
                tails[candidate.role as usize] = candidate.event;
                ordered.push(candidate.event);
            }
            ensure!(
                next == [u64::from(limit(context)) + 1; 2],
                "derived terminal next for both lanes"
            );
            contexts.push(Context {
                context: scope.context,
                scope_commitment: scope.commitment()?,
                genesis: [scope.genesis(0), scope.genesis(1)],
                count: ordered.len(),
                next,
                tails,
                ordered,
                records_commitment: *hash.finalize().as_bytes(),
            });
        }
        let genesis = self
            .scopes
            .iter()
            .flat_map(|s| [s.genesis(0), s.genesis(1)])
            .collect::<BTreeSet<_>>()
            .len();
        Ok(Terminal {
            contexts: contexts
                .try_into()
                .map_err(|_| anyhow::anyhow!("two terminal contexts"))?,
            texts: self.rows.len(),
            genesis,
        })
    }
}
