# RFC-0246: M1.2.22 authenticated history-recovery transfer inbox

## Status

Accepted locally on 2026-10-20. This is a portable offline/file boundary, not
a production network transport. Active runtime IPC remains postcard v33 with
descriptor domain v14, and the public default remains `legacy-only`.

## Problem

M1.2.21 could atomically import an old-history recovery chunk, but the source
execution journal and recipient state had to be locally accessible together.
Copying individual artifacts without another authenticated boundary would let
a future transport omit, substitute or reorder part of a committed chunk.

## Decision

The source exports only the exact current committed execution head. A bounded
canonical postcard envelope includes the source-signed execution record, the
authenticated history-rewrap bundle, both recipient-signed checkpoints and
the encrypted recipient projections. A second source Device signature binds
the complete encoded set to the owner Account, source and recipient Devices,
consent selection, execution generation and predecessor, conversation, ranges,
bundle and checkpoint IDs. Its content hash is the transfer ID.

The recipient verifies the complete envelope before mutation. It rejects a
foreign Account or recipient, non-canonical or oversized encoding, invalid
signature, substituted inner artifact, truncated input, unexpected directory
entry and symlink. The inbox then appends the exact bytes under one
state-directory transaction and lock, including the vault-primary dual-write
lifecycle when an encrypted state vault exists.

An inbox chain must begin at generation zero and advance by exactly one record,
checkpoint and range at a time. Missing generations, forks, stale alternatives
and execution-record reuse fail closed. Exact byte replay is idempotent. A
bounded linear backlog may arrive before import; the M1.2.21 recipient receipt
chain, rather than the latest inbox head, still enforces import order.

The import bridge accepts only an exact staged transfer from the same canonical
state directory. It revalidates current recipient Device and conversation
membership, then uses the existing atomic event/projection/checkpoint/receipt
transaction. Importing generation one before generation zero therefore fails,
while generations zero, one and two can be staged first and consumed later in
order. The inbox alone grants neither network nor import authority.

## Privacy and authority boundary

The envelope transports ciphertext and signed metadata. It does not store
message bodies in plaintext and its status exposes only an opaque selection
token and bounded progress. Raw identity, filesystem paths and network
coordinates are not status fields. No Iroh endpoint, HTTP client, retry loop,
multi-source scheduler, active IPC command or desktop control is added.

A source with its Device secret can authenticate only a record already
committed by M1.2.20. It cannot widen the signed consent range, change the
recipient or bypass current membership validation. A crashed append remains
behind the existing transaction marker: read-only status fails closed and a
later mutating retry performs the established recovery before staging again.

## Verification

Four stable-path tests cover export/stage/import/replay, a three-generation
backlog and out-of-order import rejection, tamper/foreign recipient/corruption,
and rollback/crash recovery. The combined execution/import/transfer harness
contains 14 passing tests. The dedicated contract verifier rejects 10/10
negative mutations. Aggregate accepted-tag readiness contains 148 closed gates
with rustfmt and workspace-wide locked/offline strict Clippy.

No product network operation, second host, release build, archive, Git tag or
automatic GitHub build was used. Whole-database rollback detection without an
external monotonic witness is not claimed. A real authenticated Iroh transfer
session, restart-safe network resume and multi-source scheduling remain future
stages.
