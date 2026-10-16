# RFC-0189: M1.1.63 dormant signed session successor store

Status: accepted locally; inactive protocol contract.

## Purpose

M1.1.62 made the first Device-signed `/cbor/2` checkpoint conditional on a
durably complete exact legacy import. It did not yet persist any successor or
prove that restart recovery validates the entire predecessor chain.

M1.1.63 extends the same dormant redb journal with atomic signed-successor
append and complete-chain restore. It changes no active CLI, Iroh endpoint,
ALPN registration or wire bytes.

## Atomic successor append

`append_session_checkpoint_successor` derives the exact session identity from
the signed successor and requires an existing durable head for that identity.
It verifies the current head record, the successor Device signature, immutable
session identity, exact sequence increment, exact predecessor checkpoint ID,
round transition, cumulative typed Event evidence, availability flags,
next-action transition and pause or terminal rules through the canonical
protocol verifier.

The successor inherits the local immutable import-plan binding from the current
head. Its immutable record and the replacement head are written in one redb
transaction with immediate durability. A byte-identical lost-response retry is
idempotent. A same-sequence fork, gap, stale successor, orphan record, terminal
successor or missing head fails closed.

## Verified restore

`session_checkpoint_chain` no longer trusts only the final head. It reads every
sequence from one through the bounded head, verifies every record key, local
import-plan binding, signature and content-derived ID, rechecks the exact
sequence-one bootstrap shape, and verifies every predecessor/successor edge.
Only then does `session_checkpoint_head` return the last record.

The defensive local bound is 65,536 signed checkpoints per session. A corrupt
head above that bound, a missing intermediate record, mutated ancestor or
broken transition is rejected rather than scanned around, repaired by guess or
silently truncated.

## Regression evidence

The store regression appends `SendInventory -> ReceiveDiff -> SendEventBatch`,
replays the last successor, reopens redb and restores the exact three-record
chain. Negative cases reject a skipped sequence, a same-sequence fork, a
successor without a durable head, a missing intermediate record and a
signed-chain ancestor mutated after the head advanced.

## Deliberate boundary

This stage supplies durable storage and verified restore primitives only. The
dormant runtime adapter still does not create successors at every frame
transition and does not resume a partially completed exchange from the restored
action. Mutual authenticated capability selection is also absent. Therefore
`kilogram/m1/sync/cbor/2` remains unregistered and undialed; Postcard v6 remains
the active protocol, with no format sniffing or parse-error fallback.

The next bounded slice integrates these primitives into every dormant client
transition and proves restart continuation. Only after that may mutual
capability selection be considered.
