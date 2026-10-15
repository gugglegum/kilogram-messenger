# RFC-0167: M1.1.41 canonical pairwise own-device ticket discovery

Status: implemented as an inactive M1.1.41 candidate.

## 1. Scope

M1.1.41 specifies deterministic-CBOR requests and Device-signed responses for
configuring and inspecting discovery of a current sibling Device's connection
ticket. Both peers already possess the exact current Root-signed Account Device list.
The mechanism does not enroll a new Device, change Account authority or
discover an unknown participant.

This slice is a local IPC contract only. It does not activate canonical CBOR,
change Postcard IPC version 26, or migrate the desktop or CLI.
It executes no network operation. Those changes require a separate explicit migration.

## 2. Configuration and capability boundary

`configure-own-device-ticket-discovery` commits:

- the exact signed Device list, authority revision and list digest;
- one active sibling Device distinct from the local descriptor Device;
- one canonical root-only HTTPS origin;
- bounded publication TTL and strictly smaller refresh lead;
- bounded announcement interval, validity, retry range and four-bit network
  mask for Ethernet, Wi-Fi, mobile and unknown networks;
- an append-only policy generation and exact expected predecessor.

The foreground runtime derives directional pairwise X25519 capabilities from
protected Device encryption material and the exact roster context. The IPC
projection contains only two distinct public channel identifiers. It contains
neither the derived read/write capability nor the Device encryption secret.

The policy ID commits the Account, local Device, exact authority state,
generation, predecessor, configuration time, full configuration and both
directional channel identifiers. Request ID is the durable idempotency key:
an exact retry may return the existing result, while divergent reuse or a stale
predecessor fails before mutation.

## 3. Verified locator and announcement handoff

The runtime publishes an opaque, Device-signed and recipient-sealed locator,
then fetches the inverse-direction record. Authentication, HPKE opening,
roster/ticket checks and rollback protection happen inside the runtime. A
successful projection reports only:

- nonzero publication generation and ordered publication/expiry timestamps;
- a bounded content-addressed ticket reference `{bytes, digest}` using the
  same artifact domain as M1.1.40;
- a nonzero authenticated observation ID;
- the child M1.1.40 announcement policy ID.

The installed locator and announcement policy ID are atomically both absent or
both present. The ticket bytes live in a trusted content-addressed staging area;
ticket plaintext, HPKE ciphertext and local filesystem paths never cross IPC.
Every later ticket use must still verify the artifact bytes and the normal
authenticated connection-ticket gate.

## 4. Derived status

`own-device-ticket-discovery-status` returns at most 32 policies in strict
recipient Device-ID order. A content-derived snapshot ID commits the authority,
current network and the complete ordered projection.

Workflow state is derived, not freely asserted:

1. `disabled` when the policy is disabled;
2. `network-blocked` when the current network is denied;
3. `backoff` after a recorded failure;
4. `awaiting-publish` before the outbound publication exists;
5. `awaiting-fetch` before a verified sibling locator exists;
6. `awaiting-announcement` before endpoint-announcement success;
7. `synchronized` after the complete publish/fetch/announce workflow.

Attempt, success, next-attempt, bundle and transport fields form exact optional
tuples with ordered timestamps. A reported success requires the installed
locator and child announcement policy.

## 5. Execution and privacy limits

The candidate runs in the foreground runtime only, requires no restart and
explicitly reports that no OS background service is enabled. Closing the
messenger stops discovery.

The HTTPS store still observes IP address, timing, record size and stable
channel use within one roster epoch. It does not receive Account or Device IDs
inside the opaque application record. This contract does not provide anonymity,
cover traffic, multi-hop relay, durable message storage or Byzantine
availability against an authorized sibling.

Root secrets, Device signing/encryption secrets, reusable publication
capabilities, ticket plaintext, HPKE envelope bytes and local filesystem paths
are forbidden from the canonical IPC objects.

## 6. Verification and next work

Rust round trips and substitution tests, checked-in golden vectors, an
independent Node implementation of deterministic CBOR/BLAKE3/Ed25519 and the
bounded decoder mutation corpus protect this boundary. The active runtime,
wire ALPN, network behavior and immutable `M1.0.0` acceptance remain unchanged.

The next bounded slice is roster-wide reconciliation: create, retain, disable
or retire pairwise policies as the exact Root roster changes, while preserving
explicit network permissions and serializing work to avoid publish/fetch storms.
