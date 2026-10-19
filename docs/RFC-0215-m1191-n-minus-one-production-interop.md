# RFC-0215: M1.1.91 N/N-1 production executable interoperability

Status: accepted from an exact two-physical-host Windows run. The public
default remains `legacy-only`.

## Decision

Kilogram accepts the exact M1.0.0 executable as N-1 and the executable built
from revision `6d95cb73359f44f20908d52b4fd2eaf7869cdede` as N. The tracked compact
record is `CANONICAL-SYNC-V2-N-MINUS-ONE-INTEROP.json`; raw logs, state,
identifiers and message bodies remain ignored local evidence.

All four bidirectional direct/relay combinations passed:

1. N initiator to N-1 responder over a direct path;
2. N-1 initiator to N responder over a direct path;
3. N initiator to N-1 responder through the relay;
4. N-1 initiator to N responder through the relay.

Every scenario included a delivered message with acknowledgement, listener
restart, seeded history synchronization and exact event-ID inventory
convergence. The negotiated common protocol was the accepted
`kilogram/m0/sync/8`; canonical v2 was not selected and no post-selection
fallback occurred.

An additional forward-upgrade chain reused the same durable state then passed
N-1/N-1, N-1/N and N/N. Event history grew from 6 to 12 to 18 events and the
last N/N exchange used a strict relay path. No downgrade after the upgrade was
attempted or required.

## Compatibility repairs

The first exact-binary attempts exposed three real interoperability defects.

First, the successor had advanced the connection-ticket envelope version and
signature domain. That prevented M1.0.0 from authenticating a new ticket before
sync negotiation could begin. The outer ticket is now frozen at the exact
M1.0.0 v11 projection. New sync capability metadata is an optional,
independently Device-signed extension. An old reader ignores the unknown JSON
field while a new reader verifies its complete binding; absence means a
legacy-only N-1 peer. The offline verifier uses the same legacy projection.
This is one v11 reader for two JSON shapes (extension absent or present), not a
version-level dual reader. The superseded experimental v13 envelope is not
accepted.

Second, the successor waited for QUIC `send.stopped()` after sending an
intermediate `SyncDiff`. M1.0.0 waits for the response FIN before opening its
required follow-up stream, so both peers could wait indefinitely. Intermediate
responses now finish their stream without treating receiver shutdown as the
receipt proof. Receipt is instead proven by the required authenticated
follow-up request on the same live connection. Final responses retain explicit
delivery confirmation.

Third, after receiving four events the exact M1.0.0 debug client needed roughly
53.8 seconds to verify, decrypt and durably store them before opening the next
inventory stream. The successor's generic 15-second stream-open timeout closed
the connection despite successful durable import. Sync follow-up acceptance now
has a bounded 120-second follow-up deadline. Frame I/O, message-size, signature,
membership and storage limits are unchanged.

## Exact evidence

The accepted M1.0.0 executable is revision
`60b20b364c8df2324a0a287b4bcb6c056ae12c39`, SHA-256
`24c5681000af166cdbe63e61942c0d0dfbc5e1fca0772e257d99dd7984cde7c6`,
57,975,296 bytes. The successor executable is revision
`6d95cb73359f44f20908d52b4fd2eaf7869cdede`, SHA-256
`22265eb35c8a94f84ab3e83c67d12235080e80c9e8f81fc56f60362897e1e13e`,
60,089,856 bytes.

Both are Windows MSVC debug executables built with at most two Cargo jobs. The
run used `DESKTOP-PAUL-RY` and `PAUL_HP_OMEN` as distinct physical hosts. No
release build, archive or tag was created.

The harness validates each copied binary hash before launch and uses stable
executable paths:

- `.tmp/kilogram-field-runner/kilogram-cli.exe` on the controller;
- `KilogramLab/n-minus-one-stable/kilogram-cli.exe` on the remote host.

This prevents a new hash-derived pathname from creating a fresh Windows
Firewall identity on every field build. A machine may still request the first
approval for the stable path or request again after local firewall policy or
network profile changes. Rust test executables must likewise be launched via
`scripts/run-cargo-tests-stable.ps1`, not directly from Cargo's hash-named
`deps` output.

## Security and rollout boundary

This evidence proves executable compatibility on the legacy common path and a
forward update of local history. It does not exercise canonical v2 between N
and N-1 because M1.0.0 cannot advertise that protocol. It does not prove every
future N/N-1 pair, performance under large histories, anonymity, or independent
security review.

M1.1.89's rollout verdict remains `hold`; M1.1.90's cohort authority remains
dormant and disconnected from runtime. This result closes the named
N/N-1-production-executable gate but does not authorize public canonical
rollout. Operational containment/controller rehearsal and independent review
remain open before an accept canary. A broad public default requires its own
later decision.

The compatible envelope also has an explicit pre-selection limitation: a
first-contact peer cannot distinguish a genuinely old M1.0.0 ticket from a new
ticket whose optional capability field was removed in transit. Existing
durable canonical selection still blocks fallback, but optional-extension
stripping before the first selection is not proven detectable. The public hold
and independent-review gate must remain until a later authority/delivery design
either closes this ambiguity or formally accepts a narrower threat model.
