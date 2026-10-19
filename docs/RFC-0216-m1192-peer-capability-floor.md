# RFC-0216: monotonic peer capability floor

Status: accepted for M1.1.92 as a dormant local decision model.

## Problem

The M1.0.0-compatible connection-ticket signature intentionally excludes the
optional sync capability. This permits an old client to authenticate the exact
legacy ticket, but it also means that a first-contact observer cannot tell a
genuine old ticket from a current ticket whose optional extension was removed.
No local parser, timeout, relay, or retry rule can recover information which
was never authenticated as required.

Kilogram therefore does not label an extension-free first contact as an old
peer. It labels it `legacy-unasserted`: usable for explicit compatibility, but
not evidence about the peer's actual software generation.

## Local states

`legacy-unasserted` has no prior authenticated floor. A future signed current
invitation may install `current-invitation-required` before its embedded ticket
is used. Only the exact canonical capability committed by that invitation can
advance to `canonical-candidate-pinned`.

Once pinned, absence of the canonical capability, regression of the Account
authority revision, or regression of the ticket publication epoch enters
`quarantined`. Quarantine never clears itself from network input; recovery
requires a separate explicit authenticated operator ceremony.

The pin intentionally records semantic high-water marks rather than an endpoint
or exact capability ID. A fresh endpoint changes the capability ID during an
ordinary restart, while its authority revision and publication epoch remain
non-regressing. The exact capability ID is required only while consuming the
one current invitation that committed to it.

## Security boundary

The pure state machine performs no I/O, persists nothing, and grants no
transport permission. Its transition must be committed atomically before a
caller relies on a raised floor. M1.1.92 does not alter the connection ticket,
sync wire, runtime IPC, or public `legacy-only` default.

This RFC does not claim universal first-contact stripping detection. Such a
claim would require prior authenticated state, a stronger invitation envelope,
or a trusted external witness. M1.1.93 will add the stronger Device-signed
invitation path; explicit M1.0.0 interoperability will remain available as
legacy compatibility rather than being misrepresented as current-peer proof.
