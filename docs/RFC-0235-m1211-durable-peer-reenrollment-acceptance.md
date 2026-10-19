# RFC-0235: Durable peer re-enrollment acceptance

Status: accepted for M1.2.11.

## Purpose

M1.2.10 deliberately stopped after producing an explicit, Device-signed peer
re-enrollment tombstone. M1.2.11 adds one narrow durable application point for
that exact artifact. It records that one replacement peer Device and one
monotonic successor capability floor were accepted, while preserving the old
quarantined capability chain as immutable audit evidence.

Acceptance is still local and offline. It does not reconnect, retry a message,
change a live runtime route, clear quarantine, alter the connection-ticket or
sync wire, or enable canonical sync for the public. The public default remains
`legacy-only`.

## Separate durable store

The pure `kilogram-peer-reenrollment` planner remains free of database and
runtime authority. Durable state belongs to the separate
`kilogram-peer-reenrollment-acceptance` crate and the separate
`peer-reenrollment-acceptance-v1.redb` database.

One immediate-durability redb transaction writes both:

- a canonical, bounded, current-local-Device-signed acceptance receipt; and
- the sole per-peer Account successor head that references that receipt.

The transaction commits both records or neither. A receipt without a head, a
head without a receipt, mismatched receipt bytes, duplicate receipt use,
corrupt metadata or owner substitution fails closed when the database opens.
A test-only failure immediately before commit proves that no partial durable
state survives restart.

The acceptance database is intentionally separate from the old capability
database. No acceptance code deletes, rewrites or lowers the old quarantine.
The old database is opened read-only by the CLI, and regression tests compare
its bytes before and after acceptance.

## Exact receipt binding

The receipt exact-binds:

- the local Account, accepting Device and exact local Root-signed authority
  revision/digest;
- the peer Account, quarantined Device, quarantine record ID, sequence, reason,
  predecessor record ID and predecessor floor;
- the complete signed tombstone through both plan ID and a domain-separated
  digest of its canonical bytes;
- the exact peer Root-signed authority revision/digest;
- replacement peer Device, current invitation ID, required capability ID,
  ticket publication epoch and successor floor;
- tombstone preparation/expiry time and local acceptance time.

The receipt is signed by the same current local Device that signed the
tombstone. Account Root keys, release-policy keys, remote peers and network
input cannot issue it. Receipt and head encodings are bounded; the store has a
bounded acceptance count.

## Replay and conflict semantics

The first valid, unexpired acceptance commits. An exact replay against the
same still-verifiable authority, quarantine and invitation context returns the
existing receipt without another write. This remains idempotent after the
short plan window expires: expiry prevents a new acceptance, not recovery from
an uncertain local result after the exact transaction already committed.

A changed authority list, invitation, quarantine context, tombstone bytes,
replacement Device or successor floor is not an exact replay. A second
successor for the same peer Account fails closed and cannot replace the first
head. Authority rollback, publication rollback, forked context, missing
quarantine and corrupt durable state remain errors.

## Manual operator boundary

The CLI exposes four explicit local commands:

1. `peer-reenrollment-plan` reads the exact Root-signed device lists,
   invitation and durable quarantine and writes a no-clobber unsigned plan;
2. `peer-reenrollment-approve` requires both `--approve` and the exact printed
   `REENROLL` phrase, then writes a no-clobber Device-signed tombstone;
3. `peer-reenrollment-accept` re-reads the complete live context and performs
   the atomic local acceptance;
4. `peer-reenrollment-status` is a read-only observer and does not create an
   absent store.

No command performs network I/O. Acceptance explicitly reports that a runtime
restart is required and that the runtime consumer is disabled. There is no
desktop one-click action, background watcher, automatic retry or hot apply in
M1.2.11.

## Verification

Stable-path Windows harnesses cover the pure plan separately from the durable
acceptance store. Acceptance tests cover atomic commit, Device signature,
bounded canonical encoding, byte-for-byte quarantine preservation, exact
replay after expiry, changed context, a second successor, injected pre-commit
failure, owner substitution, corruption and verified reopen.

The fail-closed boundary verifier protects the separate-crate architecture,
atomic receipt/head transaction, old quarantine retention, manual CLI
ceremony, disabled runtime consumer, absence of network dependencies and the
unchanged `legacy-only` public default. Aggregate readiness contains 137
closed boundary gates.

## Non-claims

M1.2.11 does not make the accepted successor usable by a running runtime,
modify contact routing, retry delivery, establish a network session, clear the
old quarantine, prove globally current peer authority, resist coordinated
whole-store rollback without an external witness, solve unknown first-contact
extension stripping, authorize public canonical rollout, or constitute an
independent security review.
