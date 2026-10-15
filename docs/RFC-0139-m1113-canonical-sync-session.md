# RFC-0139: Canonical durable synchronization session (M1.1.13)

Status: candidate, not active.

## Outcome

M1.1.13 closes the remaining canonical `sync` schema gap with a durable,
Device-signed checkpoint chain. A checkpoint binds the conversation, both
Devices, the transport-derived session binding, the current round, cumulative
received and sent Event ID evidence, availability flags, the next action and
paused or terminal state. The active Postcard v6 protocol and its ALPN remain
unchanged.

## Chain and transition rules

The first checkpoint has sequence one and no predecessor. Every successor
increments the sequence exactly once and commits to the BLAKE3 ID of the exact
previous signed checkpoint. Conversation identity, Device identities and the
session binding are immutable. Received and sent Event IDs are strictly ordered,
duplicate-free cumulative sets: a successor may add evidence but cannot remove
previously recorded evidence.

A round may remain unchanged or advance by exactly one. Advancing a round is
allowed only after `receive-complete` reports more work and must lead to
`send-inventory` without changing cumulative evidence or availability flags.
Pause and resume are explicit signed checkpoints that preserve every field
except sequence, predecessor and the paused flag. `complete` and `rejected` are
terminal, unpaused, advertise no more work and cannot have successors. Unknown
actions, non-canonical booleans, rollback, skipped generations and invalid
transitions fail closed.

## Independent conformance and bounds

An independent JavaScript implementation uses Node Ed25519 and the repository's
independent BLAKE3 reference to reproduce all four golden checkpoints: end of
round one, start of round two, pause and resume. The Rust codec verifies the
same signatures, IDs and successor relations. The signed checkpoint is bounded
to 512 KiB and each cumulative Event ID list to 4,096 entries.

The strict decoder mutation corpus grows from 3,654 to 3,915 cases under the
existing 4,000-case ceiling. With the one-round frames from M1.1.12 and this
multi-round session state, the aggregate `sync` canonical schema coverage is
complete.

## Activation boundary

This stage changes no active wire bytes and assigns no protocol identifier.
Activation still requires the migration contract, a new explicit version or
parallel ALPN, dual-reader downgrade tests, sustained fuzzing and fresh hosted
and field evidence.
