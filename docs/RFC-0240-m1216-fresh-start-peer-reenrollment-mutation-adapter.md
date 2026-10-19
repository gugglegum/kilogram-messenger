# RFC-0240: M1.2.16 fresh-start peer re-enrollment mutation adapter

Status: verified implementation boundary. This RFC does not authorize public
rollout, automatic re-enrollment, quarantine deletion, or old-history recovery.

## Purpose

M1.2.15 exposed only authenticated `inspect` and `status` operations. M1.2.16
adds a separate, authenticated runtime mutation surface for `plan`, `approve`,
and `accept`. The current Device remains the sole signing authority: the IPC
client never receives its secret, a remote peer cannot mutate local state, and
the desktop remains read-only.

The co-installed-binary boundary makes an exact hard cut from active Postcard
version 32 / descriptor domain v13 to version 33 / domain v14. There is no dual
reader, downgrade path, or permissive fallback. Connection-ticket and sync wire
formats do not change.

## Explicit authority

The three mutation steps are deliberately separate. `plan` binds the exact
opaque contact, staged invitation, live quarantine record, authority snapshots,
and replacement Device. `approve` requires both an explicit approval flag and
the exact typed `REENROLL` confirmation phrase printed by the plan. `accept`
revalidates the same live signed context before committing anything.

Observation and mutation use distinct outer IPC variants. The mutation adapter
rejects `inspect` and `status`, while the read adapter rejects `plan`, `approve`,
and `accept`. Every request is bound to the current authenticated descriptor;
every response is signed by the current Device and verified independently by
the client.

## Durable ceremony and crash recovery

The runtime owns a bounded, current-Device-signed redb journal with monotonic
phases: `planned`, `approved`, `acceptance-committed`, and `complete`. Each entry
binds the exact staged invitation, invitation, quarantine record, plan,
tombstone, acceptance receipt, and runtime transition identifiers. Exact replay
is idempotent, including a retried transition preparation with a later wall
clock; divergent reuse fails closed.

Acceptance intentionally spans more than one durable store. A crash can occur
after the acceptance receipt, after the acceptance-committed journal phase, or
after transition preparation. Those states are reported as incomplete rather
than as success. Replaying the exact `accept` operation resumes the ceremony;
it does not create a second acceptance or a different transition.

## Fresh-start application

Completion still does not hot-apply the successor. `restart_required` becomes
true only after the journal reaches `complete`. An incomplete journal is skipped
at a fresh runtime start, while the predecessor contact and quarantine remain
intact. Only a complete, exactly bound transition may be consumed at a fresh
runtime start, where queued work is rebound to the successor.

The predecessor quarantine and old readable evidence remains retained for
audit and local user interests. Old readable history is not implicitly granted
to the replacement Device; that remains a separate same-Account recovery and
rewrap ceremony.

## User surface and non-claims

The CLI provides explicit plan, approval, and acceptance commands through the
authenticated runtime descriptor. The desktop remains read-only and contains
no plan, approve, accept, clear, retry, hot-apply, or automatic-recovery action.

Stable-path Windows tests cover the complete ceremony, all three injected crash
points, honest incomplete observation, fresh-start refusal before completion,
exact replay, different-timestamp replay, final restart consumption, retained
old evidence, and read/mutation separation. This stage performs no network
operation and needs no second host. The public sync default remains
`legacy-only`; no release build, ZIP, tag, or automatic GitHub build is made.
Aggregate readiness contains 142 closed gates.
