# RFC-0230: Peer capability enforcement before outbound sync transport

Status: accepted for M1.2.6.

## Purpose

The durable monotonic peer capability floor from M1.1.94 is now enforced by
the real outbound `sync` command. After the connection ticket, Account Root
authorization and conversation membership have been authenticated, but before
an Iroh endpoint is bound or any peer connection is attempted, the client
derives capability evidence and applies it to the local Device-signed store.

An authenticated canonical candidate is durably pinned before network use. If
that extension is later missing, the store appends a quarantine record and the
command fails before endpoint creation. Network input cannot clear quarantine.
Unsupported capability sets also fail before transport selection.

## Honest legacy first contact

An extension-free ticket from a peer with no prior capability state remains
`legacy-compatible-unasserted`. It is not treated as proof that the peer is an
old client, and it does not create an otherwise absent capability database.
This preserves legacy interoperability and avoids silently enrolling every
contact merely by observing a ticket.

Consequently, generic first-contact extension stripping remains impossible to
detect. Detection requires stronger prior authenticated state: a current
invitation, an earlier signed capability observation, or a persisted mutually
signed canonical selection. This is an explicit non-claim, not a temporary
heuristic.

## Selected canonical resume

A persisted canonical selection contains the signed listener capability plus
the mutually signed offer and acceptance. For an already selected session it
is therefore stronger resume evidence than the current connection ticket. A
ticket with the optional capability removed cannot erase that evidence, force
legacy fallback, or quarantine an otherwise valid selected session.

Starting a new canonical session is stricter: it still requires the signed
canonical candidate in the current ticket as well as local activation policy.
The public policy default remains `legacy-only`, so this change cannot enroll
users into canonical sync or activate a public rollout.

## Privacy and scope

Diagnostics expose only the evidence source, semantic permitted use, bounded
record identifier and sequence, change bit and semantic floor. Ticket bodies,
endpoint addresses, relay URLs and message content are not added to this
surface. The wire formats and runtime IPC are unchanged. Automated runtime
sync, automatic quarantine recovery and public canonical activation remain
outside this milestone.
