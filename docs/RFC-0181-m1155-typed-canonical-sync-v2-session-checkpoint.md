# RFC-0181: M1.1.55 typed canonical sync v2 session checkpoint

Status: accepted locally; inactive protocol contract.

## 1. Purpose

M1.1.54 closed the history-compatible `/cbor/2` inventory, diff, Event batch,
completion, pause and rejection frames. A process interruption still had no
generation-compatible durable state: the older checkpoint stored bare
32-byte Event IDs and was bound to the closed `/cbor/1` generation.

M1.1.55 defines a separate Device-signed checkpoint chain for
`kilogram/m1/sync/cbor/2`. It does not reinterpret or mutate the v1 checkpoint.
Postcard v6 remains the only active sync protocol.

## 2. Exact generation binding

Every checkpoint content includes the exact protocol identifier byte string
`kilogram/m1/sync/cbor/2`. The identifier is inside both the Ed25519 signature
preimage and the BLAKE3 checkpoint-ID preimage. It is also immutable across a
successor chain. A missing, altered or differently sized identifier fails
closed before a checkpoint can be accepted.

The signature and checkpoint-ID domains are new and end in `cbor-v2\0`.
Consequently neither a valid `/cbor/1` checkpoint nor its signature can be
replayed as `/cbor/2` state.

## 3. Typed cumulative evidence

Received and sent evidence contains a typed Event key, encoded as a nested
deterministic-CBOR byte string. Every key carries both its explicit namespace
and its 32-byte Event ID. Bare Event IDs are not accepted.

The arrays are strictly ordered and duplicate-free by `(namespace, event-id)`.
This means the same raw 32-byte value in the legacy Postcard namespace and the
native canonical namespace remains two distinct keys; a namespace collision
cannot collapse durable evidence.

Every successor must contain all evidence from its predecessor. It may add
ordered keys but cannot remove a previously committed key, including one that
differs from another key only by namespace.

## 4. Chain and transition rules

The checkpoint sequence begins at one and increments exactly once. Sequence
one has no predecessor; later checkpoints name the exact BLAKE3 ID of the
previous signed checkpoint. Conversation, local and peer Device identities,
session binding and protocol identifier do not change.

A round may stay unchanged or increase by one. Increasing it is allowed only
after `receive-complete` reported remaining work and transitions to
`send-inventory`. Pause and resume change only the paused flag: round, action,
typed evidence and availability flags remain identical. `complete` and
`rejected` are terminal and cannot have successors.

## 5. Bounds and independent vectors

Each received or sent evidence set is limited to 4,096 typed keys. The complete
signed checkpoint is limited to 512 KiB. Unknown namespaces, invalid canonical
booleans, unknown actions, non-canonical order, rollback, skipped generations,
wrong signatures and trailing bytes fail closed.

`CANONICAL-SYNC-SESSION-V2-VECTORS.json` contains four deterministic
checkpoints: initial state, next round, pause and resume. The independent
JavaScript reference derives the Ed25519 public key, all signature preimages,
signatures and BLAKE3 checkpoint IDs without calling Rust.

## 6. Activation boundary

This stage changes no active wire bytes. `/cbor/2` remains allocated but is not
registered, advertised or dialed. Connection ticket v12 still advertises only
the active Postcard protocol and the older inactive `/cbor/1` candidate.

Activation remains blocked on an authenticated `/cbor/2` ticket capability,
dual-reader and downgrade work, sustained fuzzing, and fresh hosted and field
evidence. Format sniffing and parse-error fallback remain forbidden.
