# RFC-0265: signed recipient import-consumption response dispatcher

Status: accepted for M1.2.41; dormant signed dispatcher verified.

## Purpose

M1.2.41 composes the separately verified M1.2.38 read adapter, M1.2.39
authorization ledger and M1.2.40 crash-safe mutation adapter. The result is a
narrow dispatcher which returns only an existing M1.2.37 Device-signed response.
It remains a dormant library API: no active runtime command, socket handler,
CLI/desktop action, worker, timer or network route is registered.

## Plan boundary

`dispatch_plan` accepts only a canonical Plan request. It revalidates the
request against the exact runtime descriptor before reading any recovery state,
then executes the read-only adapter and signs its Planned or Rejected outcome
with the descriptor Device. The opaque dispatch result retains the exact plan
request and read projection for a later explicit commit. Planning creates no
authorization or mutation database and does not consume the intent.

## Commit ordering and binding

`dispatch_commit` accepts only Commit and requires the opaque result of an
earlier successful dispatcher Plan stage. Callers cannot construct or replace
that internal stage. Both the retained Plan request/response and the Commit
request are revalidated against the same descriptor. Account, Device, request
digest and current signing identity substitutions fail before mutation.

The dispatcher then durably commits the exact explicit M1.2.39 authorization,
invokes M1.2.40, and issues the signed response only after the mutation adapter
has durably stored its completed consumption receipt. A rejected mutation may
be signed as rejection, but an unsigned success never crosses this boundary.
The response contract continues to state `explicit_commit_required=true` and
`automatic_consumption=false`.

Exact Commit replay reopens the existing authorization and completed mutation,
returns the same semantic receipt as `AlreadyConsumed`, and does not call the
membership loader or consume the intent a second time. The imported history,
membership records and signed intent remain preserved.

## Verification and non-claims

Two real signed fixtures cover read-only signed Plan with no durable state,
descriptor/stage substitution rejection, ordered authorization and mutation,
signed consumed response verification, and exact replay without reload. The
formal boundary rejects 10/10 semantic mutations. Accepted-tag readiness
contains 167 closed gates with rustfmt and locked, offline strict Clippy.

This milestone does not activate runtime IPC, alter runtime IPC version 33 or
domain 14, change the `legacy-only` public default, or claim a user-facing
consumption ceremony. No network, second host, release build, archive, tag or
automatic GitHub build is used.
