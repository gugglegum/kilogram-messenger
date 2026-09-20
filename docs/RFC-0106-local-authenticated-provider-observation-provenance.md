# RFC-0106: Local authenticated provider observation provenance (M0.9.83)

Status: accepted locally after network-free verification.

## 1. Problem and claim boundary

M0.9.82 makes each fresh provider identity cost bounded work, but it cannot say
whether several qualified offers belong to different operators. Counting
transport keys alone would repeat the original Sybil assumption. M0.9.83 adds
local evidence about which already authenticated peer sessions delivered an
exact signed offer.

This evidence is not a transferable endorsement, proof of personhood or proof
of operator independence. A resourceful operator can still create multiple
Accounts and Devices. The design records a bounded local observation fact so a
later policy can combine cost, observation diversity and explicit failure-
domain evidence without pretending that any one signal solves Sybil attacks.

This limitation follows the boundary described in [John R. Douceur, *The
Sybil Attack*](https://www.microsoft.com/en-us/research/publication/the-sybil-attack/):
unknown remote identities cannot generally prove that they represent distinct
entities without an external scarcity or trust assumption.

## 2. Local pseudonymous observer tag

The runtime creates a tag only after the existing Device-authenticated Iroh
session succeeds. It derives a domain-separated local key from the current
Device signing secret with BLAKE3 `derive_key`, then applies BLAKE3 keyed mode
to the authenticated peer Account ID and Device ID. BLAKE3 specifies hash,
keyed-hash and key-derivation as separate domain-separated modes.

Only the resulting 32-byte pseudonymous tag enters the local provider registry.
The raw Account ID and Device ID do not. The tag:

- is stable for the same local Device and authenticated remote Device;
- differs for another remote Device or another local Device;
- cannot be recomputed from public contact identifiers without the local
  Device secret;
- is never encoded in a storage offer or provider gossip frame;
- is never returned through authenticated runtime IPC or printed in logs.

Manual/local offer import does not manufacture authenticated provenance. Only
the inbound or outbound provider-gossip path, after Device authorization,
supplies a tag to the registry.

Replacing the local Device secret changes the derivation epoch. A future
Device-key rotation migration must therefore clear or explicitly re-key this
local observation table; it must not silently combine tags from two local
identity epochs as if they were independent observers.

## 3. Durable bounded registry

The provider registry has a separate local-only observation table. Every row
is keyed by exact offer ID plus observer tag and records bounded first/last
observation time. Therefore repeated frames from one authenticated Device
refresh one row instead of increasing diversity.

At most eight distinct authenticated observations are retained for one exact
offer. A ninth source leaves the offer usable but produces a capacity outcome;
the registry never grows an unbounded social graph. Expired offers and replaced
signed offers lose their observation rows atomically. Provenance for an old
endpoint, capacity statement, nonce or validity window cannot migrate to a new
offer merely because the store key stayed the same.

Older registries without the observation table remain readable and expose an
observation count of zero until a writable open performs the additive schema
initialization.

## 4. Wire, IPC and selection boundary

The provider offer and gossip wire layouts are unchanged. Frames created from
registries holding different local observation sets remain byte-identical when
their signed offers, hop counts, entropy and time are identical.

Authenticated runtime IPC remains version 26. Neither observer tags nor raw
Account/Device identifiers nor observation rows/counts are projected through
the provider IPC types. Runtime logs expose only aggregate added/refreshed/
capacity counters and explicit `local-only` diagnostics.

M0.9.83 intentionally does not change replica-set ranking or require a minimum
observation count. Doing so now would lock out new users and confuse several
authenticated Devices controlled by one operator with independent failure
domains. The next policy stage must state its bootstrap behavior and combine
observation diversity with separately evidenced network/operator domains.

## 5. Verification and stage boundary

Tests cover stable/unlinkable local tag derivation, deduplication, the eight-row
bound, replacement reset, persistence, legacy-table reading and byte-identical
gossip frames despite different local observation sets. A runtime regression
proves that inbound and reply-bound outbound exchanges add or refresh evidence
only after their authenticated session context is available.

The fail-closed source boundary is
`scripts/verify-kilogram-provider-observation-boundary.ps1`. This stage is
network-free and debug-only: no field connection, release build, ZIP, new
executable, background service or external publication is created.

Local acceptance covered 16 mailbox-client tests, three focused CLI runtime
tests, all 12 mailbox/provider/service-free static boundaries, formatting and
Clippy with warnings denied, using at most two Cargo jobs.
