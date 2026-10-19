# RFC-0242: Explicit old-history recovery plan after peer re-enrollment

Status: accepted locally for M1.2.18.

## Purpose

A successfully re-enrolled replacement Device can receive new messages, but it
must not thereby gain the ability to decrypt old pairwise ciphertext. M1.2.18
adds a separate offline planning boundary for restoring readable history from
an already authorized Device that still has local projections. It builds on
the existing same-account authenticated history-rewrap and checkpoint
protocols; it does not create a second encryption or transfer format.

Re-enrollment approval, peer acceptance and old-history recovery consent are
three distinct acts. The first two never imply the third.

## Read-only inventory

Each source Device signs a bounded claim containing only:

- its Account and exact Root-signed authority revision/digest;
- conversation IDs, event counts and canonical inventory digests;
- readable index ranges for which local projections exist.

The claim type has no message-body field. At most eight sources, 64
conversations, 256 ranges and 256 existing checkpoints may enter one snapshot.
Existing checkpoints are verified with the established recipient-Device
signature and are accepted only for the exact Account, replacement Device,
source, conversation, inventory claim and readable range.

The UI-facing projection replaces conversation IDs with scoped opaque tokens
and exposes only bounded counts, range/checkpoint counts and `Agreed` or
`Divergent`. It contains no message body, identity secret, filesystem path,
ticket or endpoint. If two sources claim different event count/digest for the
same conversation, divergence remains visible and that conversation cannot be
planned.

## Exact re-enrollment binding

The recovery binding may be constructed only from an already verified
acceptance receipt, the exact durable runtime transition and the current peer
authority. The transition must have been consumed by a fresh runtime. Receipt,
transition, local Account/Device, peer Account, quarantined predecessor,
replacement Device and authority revision/digest must all match.

This check prevents a generic inventory or a plan for a different successor
from being relabeled as recovery for the current peer transition.

## Content-addressed plan and consent

One plan chooses one source Device and one or more canonical conversation
ranges. It binds:

- exact acceptance receipt and consumed transition IDs;
- replacement Device and peer authority revision/digest;
- complete inventory ID and selected signed source-claim ID;
- exact membership revision, owner and artifact digest for every conversation;
- source inventory count/digest and selected half-open range;
- exact existing checkpoint ID and next index, when present;
- preparation time, 15-minute expiry and fresh nonce.

The plan is content-addressed before approval. Creating it grants no recovery
authority. The source Device that owns the readable projections must then
separately sign it after the user supplies the exact `RECOVER HISTORY ...`
phrase and an explicit history-owner approval flag. The replacement Device
cannot self-approve source history.

Any changed source inventory, authority, membership, recipient, range,
checkpoint or re-enrollment transition fails exact context verification.

## Deliberate non-execution boundary

M1.2.18 does not read plaintext, build a rewrap bundle, advance a checkpoint,
write a database, contact another Device or add a desktop/runtime command. A
signed plan is only a future bounded consent artifact. Transfer execution,
durable consent persistence, retry scheduling, resumable multi-source work and
final claim reconciliation remain later stages.

The active IPC remains Postcard v33 with descriptor domain v14. Connection
ticket and sync wire formats are unchanged and the public default remains
`legacy-only`.

## Verification

Six pure planner tests cover body-free inventory projection, source agreement,
existing checkpoint binding, explicit source-owner consent, content addressing,
divergence, context drift, expiry and tampering. On Windows the harness is
compiled without execution by Cargo and run only from the stable-name test
path required by the project firewall policy.

The dedicated fail-closed boundary validates the manifest, source/API surface,
absence of network/storage/desktop dependencies and ten negative mutations.
Aggregate accepted-tag readiness contains 144 closed gates with rustfmt and
workspace-wide locked/offline strict Clippy.

No second host, product network, release build, archive, Git tag or automatic
GitHub build is used by this stage.
