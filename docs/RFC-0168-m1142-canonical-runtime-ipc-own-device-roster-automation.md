# RFC-0168: M1.1.42 canonical roster-wide own-device automation

Status: implemented as an inactive M1.1.42 candidate.

## 1. Scope

M1.1.42 specifies deterministic-CBOR requests and Device-signed responses for
reconciling one exact Root-signed roster into all pairwise own-device ticket
discovery policies. It builds on M1.1.41 without enrolling a Device, changing
Account authority or accepting an untrusted roster.

This slice is a local IPC contract only. It does not activate canonical CBOR,
change Postcard IPC version 26, or migrate the desktop or CLI. It executes no network operation.
Those changes require a separate explicit migration.

## 2. Parent policy

`configure-own-device-roster-automation` commits:

- the exact Root-signed Account Device list, its authority revision and its
  domain-separated signed-list digest;
- one canonical root-only HTTPS origin;
- bounded publication TTL, refresh lead, announcement timing and retry range;
- an explicit four-bit Ethernet, Wi-Fi, mobile and unknown-network mask;
- an append-only parent generation and exact expected predecessor.

The request ID is the durable idempotency key. An exact repeat may return the
already committed head, while divergent reuse or a stale predecessor fails
before any policy mutation.

## 3. Exact child projection

The runtime derives the child set from the supplied authority state. It contains
exactly every active sibling Device and every retained, nonlocal permanently
revoked Device. Active children inherit the parent configuration. Each retired tombstone
is retained for audit and forced disabled so a revoked Device cannot
silently return through stale local state.

Children are unique, limited to 64 and emitted in strict Device-ID order. The
runtime recomputes every M1.1.41 child policy ID from the Account, local Device,
authority state, derived configuration, generation, predecessor, configuration
time and public directional channel identifiers. Omission, duplication,
reordering, an unknown recipient or a mismatched child fails closed.

The child discovery status remains its own canonical CBOR object embedded in a
byte string. It is independently decoded and validated. This preserves the
global canonical-CBOR nesting limit instead of weakening it for the roster.

## 4. Atomic status and serialized work

One snapshot ID commits the Account, local Device, authority revision, signed
list digest, current network, complete parent and complete ordered child set.
Declared active and retired counts must exactly match that set.

Reconciliation permits at most one foreground workflow. This deliberately
serializes publication, fetch and announcement work to avoid a connection or
HTTPS storm when a large roster changes. The contract describes desired and
observed policy state; it does not promise that every sibling is currently
online or that the opaque-ticket store is available.

## 5. Privacy and execution boundary

The projection exposes public channel identifiers, content-addressed ticket
references and bounded scheduler state. Account Root secrets, Device signing or
encryption secrets and reusable derived capabilities never cross the boundary.
HPKE ciphertext, ticket plaintext and local filesystem paths are equally forbidden.

The candidate requires no restart, runs as one foreground workflow only and
explicitly reports that no OS background service is enabled. Closing the
messenger stops reconciliation. The HTTPS store can still observe IP address,
timing, sizes and channel use; this contract does not provide anonymity or
Byzantine availability.

## 6. Verification and next work

Rust round trips and substitution tests, checked-in golden vectors, an
independent Node implementation of deterministic CBOR/BLAKE3/Ed25519 and the
bounded decoder mutation corpus protect the boundary. The active runtime, wire
ALPN, network behavior and immutable `M1.0.0` acceptance remain unchanged.

The next bounded runtime-IPC slice is the recovery-plan and device-link command
family, followed by the active unpaged conversation-list migration. Activation
of any candidate remains a separate, explicit compatibility decision.
