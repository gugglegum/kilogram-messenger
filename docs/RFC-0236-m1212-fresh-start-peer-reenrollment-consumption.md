# RFC-0236: Fresh-start peer re-enrollment consumption

Status: accepted for M1.2.12.

## Purpose

M1.2.11 durably accepted exactly one replacement peer Device, but deliberately
left the active runtime contact unchanged. M1.2.12 adds the narrow consumer for
that accepted successor. The transition can be prepared by the explicit
acceptance ceremony, but it becomes active only on a later fresh runtime start
and before any network endpoint is created.

The public default remains `legacy-only`. This stage does not change the
connection-ticket, sync or runtime IPC wire, clear old quarantine, send a
queued message automatically, or claim a two-host field result.

## Separate two-phase transition store

The new `kilogram-peer-reenrollment-runtime` crate owns a separate bounded
`peer-reenrollment-runtime-v1.redb` database. It has no Iroh, Tokio, HTTP,
runtime IPC, desktop or CLI dependency. A current local Device signs the
complete transition before it is persisted.

There are two durable phases:

1. `prepared` contains the complete immutable successor transition;
2. `consumed` maps the logical contact ID to the exact prepared transition ID.

Both writes use immediate durability. Preparing the exact same transition and
consuming the exact same transition are idempotent. A changed successor for an
already prepared contact, a different expected transition ID, corrupt state,
foreign owner metadata or a conflicting consumed head fails closed. An
injected failure before prepare commit leaves neither a prepared record nor a
consumed head after reopen.

## Exact transition binding

The signed transition binds:

- local Account and current local Device;
- peer Account, old quarantined Device and replacement Device;
- exact acceptance receipt ID and retained quarantine record ID;
- stable logical contact and conversation IDs;
- a domain-separated digest of the byte-exact predecessor contact;
- complete signed successor contact and enrolled endpoint candidate bytes,
  with independent domain-separated digests;
- complete canonical accepted invitation bytes and invitation ID;
- the non-regressing successor capability floor and preparation time.

Before preparation, the CLI revalidates the acceptance against both current
Root-signed authority snapshots, the retained quarantine and the exact
invitation. The replacement descriptor file must contain the accepted ticket
bytes. Ticket authentication remains checkable after invitation expiry so a
crash after durable acceptance cannot make exact recovery impossible; expiry
still prevents a new acceptance.

## Fresh-start boundary

A prepared transition alone does not change the runtime snapshot. Runtime
startup first loads the unmodified predecessor state and preflights the
complete set of transitions. Every prepared transition must still match its
predecessor contact, enrolled candidate, descriptor bytes, acceptance receipt,
retained quarantine and the exact local and peer authority accepted by the
ceremony. A corrupt second transition therefore prevents consumption of the
first.

Only after the complete preflight succeeds are consumed heads committed. This
happens after local listener preparation but before construction or binding of
the Iroh endpoint. Exact restart replay returns `already-consumed` and does not
replace the head.

Once consumed, the receipt remains immutable evidence rather than a permanent
lock to an obsolete authority revision. Monotonic authority advances after
consumption are accepted, while rollback and same-revision equivocation still
fail closed. The replacement certificate is rechecked against the newest pinned
peer authority on every replay, so its later revocation also fails closed.

Subsequent snapshot loads apply only consumed transitions as an in-memory
overlay: the stable logical contact now points at the replacement Device and
the separately enrolled replacement candidate is removed from the in-memory
candidate set. The original predecessor contact file is not overwritten.

## Retained evidence and queue semantics

The predecessor contact file, old quarantine chain, acceptance receipt,
prepared transition and candidate source artifact remain durable audit
evidence. No clear, rewrite or delete authority is added.

An already queued item stays sealed and unmaterialized during transition
consumption. Consumption does not start networking or delivery. On the normal
runtime loop, the ordinary conversation-scoped capability gate and scheduler
must reevaluate the item against the replacement contact. This avoids a hidden
delivery side effect and preserves the existing retry/ACK discipline.

## Verification

Five stable-path store tests cover prepare/consume/reopen, exact replay,
changed successor conflict, failure immediately before commit, wrong expected
transition ID and corrupt-record reopen. A CLI integration test creates a real
old contact, durable quarantine, accepted replacement and queued item. It
proves prepared-only invisibility, fresh-start activation, byte-exact retained
predecessor and quarantine, unmaterialized queue retention, idempotent restart
after monotonic local and peer authority advances, and fail-closed descriptor
substitution.

The fail-closed boundary protects the separate store, complete binding,
preflight-before-consume order, consumption before network endpoint creation,
retained evidence, no automatic delivery and unchanged public default.
Aggregate readiness contains 138 closed boundary gates.

## Non-claims

M1.2.12 does not prove the replacement route on two physical hosts, guarantee
automatic delivery immediately after re-enrollment, delete the predecessor or
quarantine, prove globally current peer authority, resist coordinated
whole-store rollback without an external witness, solve unknown first-contact
extension stripping, authorize public canonical rollout, or constitute an
independent security review.
