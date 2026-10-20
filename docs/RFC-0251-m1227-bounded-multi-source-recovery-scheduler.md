# RFC-0251: M1.2.27 bounded multi-source recovery scheduler

## Status

Accepted locally on 2026-10-20. This is a dormant, explicitly invoked
source-side coordination primitive over already durable transfer intents. It
is not registered by the active runtime, CLI or desktop application. Runtime
IPC remains postcard v33 with descriptor domain v14, and the public default
remains `legacy-only`.

## Problem

M1.2.25 executes one source transfer and M1.2.26 accepts a finite recipient
budget, but naïvely retrying one unavailable source until exhaustion can starve
other sources that are ready. Conversely, an unconstrained concurrent fan-out
would multiply endpoint, bandwidth and ambiguity costs and could become a
background recovery service.

## Decision

The scheduler accepts only a caller-supplied slice of one to eight already
durable transfer intents. Before the first network attempt it loads and
revalidates every request, both signed Endpoint bindings and its current outbox
status. All entries must belong to the same Account and recipient Device, while
source Devices and transfer IDs must be distinct. One bad late entry therefore
fails before an earlier entry can open the network.

The default policy permits three rounds and 24 total executor invocations. The
hard limits are eight rounds and 64 total invocations. In each round, at most
one attempt is made for each still-pending source, in exact caller input order.
There is exactly one invocation in flight. A retryable pre-connect or
post-connect-ambiguous result yields immediately to the next source instead of
consuming that source's full retry budget first. A round starts only when the
remaining total budget can cover every source pending at its beginning; a
partial round can therefore never favor an earlier input.

Every network attempt delegates to the M1.2.25 executor with a one-attempt
policy. The scheduler therefore cannot create a request, choose another nonce,
change an Endpoint binding or acknowledge before the verified receipt is
durable. A post-connect-ambiguous retry uses the exact request stored by the
outbox. An already acknowledged source is skipped without network.

Round or total-attempt exhaustion leaves all unfinished exact intents pending
for a later explicit invocation. Unexpected local, binding or outbox failure
stops the invocation fail closed; receipts completed for earlier sources remain
durable and will be skipped on restart. The scheduler itself persists no retry
state, owns no timer and performs no history import.

## Observation and authority boundary

The report identifies entries only by their ordinal in the caller-supplied
slice. It exposes bounded attempts, closed outcomes and closed failure classes,
but no Account, Device, Endpoint, request or transfer ID, network coordinate,
path or message body. It grants no import authority.

This stage does not discover remote sources or coordinate processes across the
network. A caller must already possess each source's local Endpoint, outbox and
exact target. There is no active runtime registration, CLI command, desktop
control, spawned task, background worker or persistent schedule.

## Verification

Seven focused stable-path tests cover policy limits; deterministic round-robin
fairness; total-attempt termination; skipping acknowledged sources; resumable
round exhaustion; empty and unexpected-failure fail-closed behavior; and full
durable candidate preflight for endpoint substitution and duplicate sources
without opening a connection.

The scheduler adds no new wire primitive. Its delegated one-transfer network
path is the real local Iroh loopback already exercised by M1.2.25, while the
new fairness state machine is deterministic. Together with transport, outbox,
executor and acceptor tests, the crate passes 26/26. The dedicated boundary
rejects 10/10 negative mutations. Aggregate accepted-tag readiness contains
153 closed gates with rustfmt and workspace-wide locked/offline strict Clippy.

No product Internet, relay or second-host claim is made. No release build,
archive, Git tag or automatic GitHub build is used.

## Next

M1.2.28 should persist a recipient-side recovery coordinator that reconciles
the approved source inventory, staged transfer backlog and import receipts. It
must make restart progress explicit without auto-importing, opening the network
or granting active runtime/desktop authority.
