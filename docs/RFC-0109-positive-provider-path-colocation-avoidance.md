# RFC-0109: Positive provider path co-location avoidance (M0.9.86)

Status: accepted locally after network-free verification.

## 1. Purpose and claim boundary

M0.9.85 records an installation-local keyed tag for the selected direct IP or
relay origin after a verified volunteer-mailbox exchange. M0.9.86 uses only the
positive meaning of that evidence: two exact offers with equal kind, derivation
epoch and tag are known to have shared one locally observed path domain.

The policy avoids choosing a second such offer while another candidate remains.
It is a preference, not an admission rule or availability gate. It does not
claim that unequal or missing tags prove independent operators, networks,
machines, ASNs, locations or failure domains.

## 2. Selection policy

New replica-set provisioning, automatic legacy upgrade and diagnostic provider
selection retain the M0.9.84 ordering and apply the following bounded algorithm:

1. Exclude offers below the existing 18-bit admission-work floor.
2. Rank exact offers by the existing binary authenticated-observation class and
   salted deterministic rendezvous rank. Observation count above one still adds
   no weight.
3. In the first pass, retain exact transport-identity deduplication and defer an
   offer only when it positively shares a verified path-domain tag and epoch
   with an already selected offer.
4. A candidate with no path-domain evidence remains eligible in the first pass.
   A different known tag receives no independence bonus; it merely lacks the
   positive equality signal that would defer it.
5. If non-conflicting and unknown candidates cannot fill the requested count,
   reconsider deferred co-located offers in their original ranked order until
   the request is filled or all transport-distinct candidates are exhausted.

The second pass preserves availability when every known provider used one relay
or IP. The first pass lets an unknown candidate displace a known duplicate, so
bootstrap does not require a prior successful exchange with every provider.

## 3. Scope and lifecycle

Only creation of a new exact replica commitment and the corresponding automatic
legacy-upgrade/diagnostic selection consume this preference. An existing exact
recipient-authenticated replica set continues to resolve its committed store
keys directly. Retrieval, delete, receipt thresholds, gossip eligibility and
wire formats are unchanged.

Path evidence remains bound to one exact signed offer. Replacement or expiry
removes it; selection then treats the replacement as path-unknown until a new
verified exchange records fresh evidence. A changed selected path replaces the
old observation rather than accumulating an unbounded topology history.

## 4. Privacy and security boundary

Selection compares opaque local values only through
`shares_verified_path_domain_with`. It does not reveal tag, epoch, path kind,
raw IP or relay URL through provider offers, gossip, protocol messages, logs or
authenticated runtime IPC v26. Operator-facing output states only constant
policy booleans.

Equal tags are useful positive co-location evidence. Unequal tags are not a
proof of independence: one operator can use multiple IPs or relays, and several
operators can share a NAT, VPN or relay. Missing evidence is unknown, never
unsafe or rejected. Admission work, authenticated observation and path
co-location avoidance raise the cost and reduce one visible concentration
case, but do not make the provider set Sybil-proof.

## 5. Verification and stage boundary

Library regression constructs the two highest rendezvous-ranked offers with one
equal local path-domain tag. A two-provider request selects the first offer and
the next path-unknown candidate; a request for every offer eventually restores
the deferred co-located candidate and remains transport-distinct. Repeated
selection is deterministic.

The fail-closed source contract is
`scripts/verify-kilogram-provider-colocation-avoidance-boundary.ps1`. It checks
the two-pass preference, unknown fallback, preserved admission/corroboration and
transport rules, exact-commitment compatibility, privacy exclusions and
unchanged IPC version.

This stage is network-free and debug-only. It creates no release build, ZIP,
new executable, background process, field connection, external publication or
Git tag.
