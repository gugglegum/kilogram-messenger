# RFC-0250: M1.2.26 explicit bounded recipient acceptor

## Status

Accepted locally on 2026-10-20. This is a finite, explicitly invoked wrapper
around the existing authenticated single-session recipient handler. It is not
registered by the active runtime, CLI or desktop application. Runtime IPC
remains postcard v33 with descriptor domain v14, and the public default
remains `legacy-only`.

## Problem

M1.2.25 can explicitly deliver one durable request, but a recipient caller
would still need to assemble its own accept loop. An unbounded loop, detached
task or shared endpoint could silently turn test-only transfer authority into a
long-lived service. A malformed or stalled peer could also monopolize a
single-session wrapper indefinitely.

## Decision

The recipient acceptor borrows a caller-owned endpoint and processes a finite
policy-selected number of sessions. The default is one session and the hard
cap is eight. It accepts sequentially, with exactly one session in flight, and
returns after its session budget, idle deadline or endpoint closure. It never
spawns a task and has no lifetime outside the awaited function call.

Only `kilogram/m1/history-recovery/postcard/1` is accepted. Handshake and
request work have separate bounded deadlines from the durable completion step:
the defaults are 30, 30 and 210 seconds respectively, and every configured
deadline is capped at 300 seconds. This keeps an unauthenticated stalled
request cheap without truncating the already bounded durable inbox stage.

Every accepted connection consumes one slot, including wrong-ALPN, malformed,
rejected and timed-out sessions. Such a failure is closed explicitly and does
not end the remaining budget. A later session can therefore proceed, while an
attacker cannot obtain unlimited attempts from one invocation. Success is
reported only after the existing handler durably stages the exact envelope and
signs its response. The acceptor never imports history.

## Observation and authority boundary

The result exposes only bounded counts, a closed failure class and a closed
termination reason. It contains no Device or Endpoint identity, network
coordinate, filesystem path or message body, and grants no import authority.

This stage adds no active runtime registration, CLI command, desktop control,
background service, spawned task, persistent schedule or multi-source planner.
Supplying the endpoint, recipient identity, inbox and policy is explicit caller
authority for only that invocation. Active applications still do not depend on
the dormant transport crate.

## Verification

Five focused tests cover strict policy bounds; idle termination and privacy;
one real authenticated Iroh loopback transfer; deterministic continuation
after wrong-ALPN and malformed/rejected sessions; and continuation after a
request timeout. The real transport test uses one exact `127.0.0.1` session.
The multi-session control-flow tests are deterministic because rapidly
replacing a deliberately stalled local Iroh connection is transport-cache
sensitive and is not evidence of product routing behavior.

Together with the previous transport, outbox and executor tests, the crate
passes 19/19. The dedicated boundary rejects 10/10 negative mutations.
Aggregate accepted-tag readiness contains 152 closed gates with rustfmt and
workspace-wide locked/offline strict Clippy. Rust network tests are compiled
without execution and launched only through the stable Windows executable
path.

No product Internet, relay or second-host claim is made. No release build,
archive, Git tag or automatic GitHub build is used.

## Next

M1.2.27 should add a bounded, explicit multi-source recovery scheduler over
already durable transfer intents and caller-supplied source candidates. It
must preserve per-source isolation, global budgets and deterministic progress
without introducing a background worker, active runtime authority or implicit
history import.
