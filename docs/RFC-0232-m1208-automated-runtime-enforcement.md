# RFC-0232: Peer capability enforcement in automated runtime work

Status: accepted for M1.2.8.

## Purpose

M1.2.6 put the durable peer capability floor in the foreground `sync` path,
and M1.2.7 verified that boundary on two physical Windows hosts. This milestone
applies the same decision to both kinds of background outbound work performed
by the ordinary runtime: queued message delivery and scheduled contact sync.

There is still one semantic authority path. Runtime work resolves its
authenticated endpoint candidates and calls the same conversation-scoped
capability authorizer used by foreground sync. A persisted mutually signed
canonical selection remains stronger resume evidence than the current optional
ticket extension. An unknown extension-free first contact remains
`legacy-compatible-unasserted` and does not create an otherwise absent
capability database.

## Outbox ordering

For a queued message, membership and endpoint-candidate authority are checked
first. The capability floor is then evaluated for every candidate before the
message body is opened, ratchet output or an event is materialized, an endpoint
is contacted, or mailbox fallback can run. A quarantined peer therefore leaves
the sealed queue item intact and creates no delivery retry record.

The runtime remembers that queue item only for the lifetime of the process, so
it cannot become a busy loop. After restart it deliberately checks the durable
floor again instead of trusting ephemeral memory; the result remains blocked
until a future explicit recovery design says otherwise.

## Scheduled sync ordering and fairness

Scheduled sync performs the same all-candidate preflight before invoking the
existing sync command. A capability block stops candidate failover and marks
that contact as process-locally blocked. Later scheduler cycles may select other
due contacts, so one quarantined peer does not starve unrelated peers.

Restart clears only the scheduler cache. It does not clear the Device-signed
capability record, and the first later attempt re-evaluates that durable record
before network use. Tests cover this restart behavior and multi-contact
selection.

## Bounded observation

The runtime emits explicit `capability-blocked` status, the bounded semantic
quarantine reason, and `network_attempted=false` when preflight stops the work.
It does not print a ticket body, endpoint address, relay URL, or message body.
No retry or mailbox failure is fabricated for a policy refusal.

These diagnostics are process output, not a new mutable runtime IPC authority.
Read-only desktop/IPC presentation is intentionally deferred to a later slice.

## Verification

The focused stable-path harness passed five peer-capability tests plus the
runtime outbox/restart and multi-contact scheduler tests. The complete CLI
stable-path regression then passed 99/99 binary tests and 1/1 library test.
Aggregate accepted-tag readiness passed 134/134 with rustfmt and
workspace-wide locked/offline strict Clippy. No release build, archive, tag or
second-host run was needed because this milestone changes no wire format.

## Scope and non-claims

Connection-ticket wire, sync wire and runtime IPC are unchanged. There is no
automatic quarantine recovery, no remote clear operation and no automatic
canonical enrollment. The public policy default remains `legacy-only`.
Generic first-contact extension stripping and whole-store rollback without an
external witness remain explicit non-claims. This milestone is local runtime
enforcement evidence, not an independent security review or permission to
activate public canonical rollout.
