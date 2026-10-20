# RFC-0249: M1.2.25 explicit restart-safe history-transfer executor

## Status

Accepted locally on 2026-10-20. This is an explicitly invoked source-side
executor for one already durable transfer intent. It is not registered by the
active runtime, CLI or desktop application. Runtime IPC remains postcard v33
with descriptor domain v14, and the public default remains `legacy-only`.

## Problem

M1.2.24 preserves one exact signed request and its eventual receipt, but callers
would still need to assemble connect, retry and commit behavior themselves. A
caller that re-prepared after a timeout could accidentally change the nonce,
Endpoint binding or request identity. It could also report success after seeing
a receipt but before that receipt became durable.

## Decision

The new executor accepts only a transfer ID already present in the M1.2.24
outbox. It loads and revalidates the exact canonical request, checks that the
caller-owned local Iroh Endpoint and target `EndpointAddr` match the two signed
Endpoint bindings, and only then permits a network attempt. It offers exactly
`kilogram/m1/history-recovery/postcard/1`; there is no legacy or alternate-ALPN
fallback.

Every retry clones the same verified prepared object and sends its exact stored
bytes. The executor cannot sign or prepare a request, choose another nonce, or
replace the durable transfer head. A verified recipient-Device-signed receipt
is committed through the immediate-durability outbox before success is
returned. If the receipt commit fails, the intent remains safely retryable.

The policy defaults to three attempts and is hard-capped at eight. Zero attempts
and a zero connect timeout are rejected. A failure before an authenticated Iroh
connection is `pre-connect`, which proves that this attempt could not deliver
application bytes. Once a connection exists, every later failure is
conservatively classified `post-connect-ambiguous`, even if a lower layer says
that stream opening failed. Ambiguity is safe because the exact request is
idempotently staged by the recipient.

Exhausting either class does not mutate or remove the pending intent. A later
explicit invocation, including after process restart, loads the same signed
bytes, nonce, request ID and transfer ID. If an acknowledgement is already
durable, the executor returns `AlreadyAcknowledgedWithoutNetwork` before any
connect or stream call.

## Authority boundary

This stage adds no loop, timer, persistent retry schedule, background worker,
recipient accept service, multi-source planner, active runtime IPC command or
desktop control. Supplying an `Endpoint`, target address and policy is explicit
caller authority for only this invocation. The outbox status remains body-free
and does not expose network coordinates.

The source executor does not import recipient history and a transport receipt
does not grant import authority. Recipient import remains controlled by the
existing recipient-Device-signed import receipt chain.

## Verification

Six focused stable-path tests cover strict policy bounds; exact bytes across a
pre-connect failure, an ambiguous failure and eventual success; pending state
across separate exhausted invocations; no-network acknowledged short circuit;
Endpoint substitution before an attempt; and a real host-local Iroh exchange
over the dedicated ALPN. Together with the previous transport and outbox tests,
the crate passes 14/14.

The dedicated boundary rejects 10/10 negative mutations. Aggregate accepted-tag
readiness contains 151 closed gates with rustfmt and workspace-wide
locked/offline strict Clippy. Rust network tests are compiled without execution
and launched only through the stable Windows executable path.

The network evidence is exact `127.0.0.1` loopback, not a product Internet,
relay or second-host claim. No release build, archive, Git tag or automatic
GitHub build is used.

## Next

M1.2.26 should add a bounded, explicitly invoked recipient acceptor around the
existing single-session handler. It must authenticate and process only a
caller-selected number of sessions, expose no background lifetime, and remain
dormant outside tests. Multi-source recovery remains a later separately gated
stage.
