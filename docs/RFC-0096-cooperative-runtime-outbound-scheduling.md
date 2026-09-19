# RFC-0096: Cooperative runtime outbound scheduling (M0.9.73)

Status: implemented locally; a fresh M0.9.73 replacement field kit is defined
and its external run is pending.

## 1. Problem

The first clean M0.9.72 field run reached the same pinned relay from both
networks and Bob reached both volunteer providers, but Alice and Bob could not
finish their authenticated peer sessions. The runtime kept one Iroh accept
future alive across timer ticks, yet the body of a tick awaited capability
push, delivery, mailbox work and automatic sync inline. While one of those
network waits was pending, the accept future and authenticated IPC were not
polled.

Two peers with similar startup schedules could therefore both wait for an
outbound response while neither serviced the other peer's inbound request.
The failure was a runtime scheduling lockstep, not missing relay reachability
or missing provider offers.

## 2. Runtime actor boundary

The runtime now owns at most one `RuntimeOutboundCycleTask`. A tick starts the
task only when no previous cycle is active. The main actor continues polling:

- the persistent authenticated Iroh accept future;
- Ctrl+C and idle shutdown;
- completion of the single outbound task;
- authenticated IPC between outbound cycles.

The outbound task retains all scheduler timestamps and retry maps and returns
them with a bounded completion report. It does not create an unbounded worker
pool. Existing per-state-directory locking serializes its local mutations with
an accepted application session. IPC mutation is deliberately admitted only
between outbound cycles, so a synchronous IPC handler cannot race a vault
dual-write owned by the background cycle.

On graceful IPC shutdown there is no active outbound task because IPC is
admitted only between cycles. Ctrl+C may abort one task; the existing state
transaction and vault recovery rules remain the crash boundary.

## 3. Deterministic automatic-sync initiator

Synchronization is bidirectional, so two simultaneous connections are not
needed. For every contact exactly one endpoint initiates automatic sync:

1. different accounts compare their 32-byte Account IDs;
2. the lexicographically smaller Account ID initiates;
3. devices in the same account use Device ID as the tie-break.

The other endpoint records the interval attempt as `runtime_sync_role=passive`
and remains available to accept the elected endpoint. This removes symmetric
sync lockstep while preserving manual sync and bidirectional event exchange.

The election is only a scheduling rule. It grants no authority and changes no
signed protocol artifact.

## 4. Invariants

- exactly zero or one outbound cycle task exists per runtime;
- a second tick never starts another cycle while one is active;
- the Iroh accept future remains alive and pollable during outbound network
  waits;
- accepted application sessions still pass through the existing authenticated
  session and state-lock boundary;
- IPC work is not dispatched while the outbound task owns mutable state;
- auto-sync elects exactly one initiator for a reciprocal contact pair;
- no new executable, server, discovery authority or wire-format version is
  introduced.

## 5. Verification

The regression suite contains:

- a pure election test proving opposite peers cannot both be initiator or both
  be passive;
- a two-runtime test with mutual contacts and auto-sync enabled on both sides;
  each runtime is allowed exactly one terminal action/session, proving the
  elected initiator completes while the peer services the inbound connection;
- the existing durable outbox plus automatic-sync convergence test.

`scripts/verify-kilogram-runtime-cooperative-scheduling.ps1` pins these source
and test boundaries. A successful local regression is necessary but not a
replacement for a fresh two-network no-HTTPS field run.

The replacement harness uses the complete M0.9.72 no-HTTPS evidence contract
with a distinct `M0.9.73` build milestone and `m0973` conversation/message
prefix. `scripts/verify-kilogram-m0973-no-https-kit-boundary.ps1` requires this
identity, the cooperative-scheduling boundary, no HTTPS fixture, debug-only
compilation and the same six operator launches. A failed M0.9.72 run cannot be resumed or relabelled as M0.9.73 evidence.

## 6. Remaining limits

This stage deliberately keeps one outbound workflow per runtime. A long
automatic sync can delay IPC until that cycle finishes, and Ctrl+C can leave a
recoverable pending vault intent. Future work may split network preparation,
exchange and commit into explicit actor messages, but it must preserve the
single-writer and fail-closed vault invariants established here.
