# RFC-0234: Device-authorized peer re-enrollment plan

Status: accepted for M1.2.10.

## Purpose

Durable capability quarantine must not become either a permanent account-loss
trap or a remotely clearable warning. M1.2.10 introduces the narrow bridge
between those two unsafe extremes: an offline re-enrollment plan that a current
local Device can explicitly inspect, confirm and sign.

The ceremony does not clear or rewrite the quarantined chain. It produces a
bounded Device-signed tombstone plan that names the exact old quarantine head
and the exact successor invitation. Durable application is a separate later
milestone. The connection-ticket wire, sync wire, runtime IPC and public
default remain unchanged; the public default remains `legacy-only`.

## Verified quarantine context

The peer capability store exposes a strictly read-only quarantine context. It
contains the exact peer key, signed head, quarantine reason and immediately
preceding signed floor. Opening that context verifies the complete retained
chain, record IDs, owner Device, sequence continuity and predecessor type.

The predecessor is required because a quarantine record intentionally stores
only its bounded reason. Its verified predecessor preserves the last
authenticated Account-authority revision and ticket-publication high-water
marks. The reader never creates a store and never appends, replaces or deletes
a record.

## Authority prerequisites

Preparing a plan requires all of the following at one explicit local instant:

- the exact current local Root-signed Account device list, containing the
  signing Device with history-sync capability;
- the exact verified quarantine context;
- the exact current peer Root-signed Account device list;
- a fresh current peer invitation signed by an active peer Device with
  history-sync capability and addressed to the local Account;
- an exact peer authority revision shared by the invitation and device list;
- a non-regressing authority high-water mark and, for the same peer Device, a
  non-regressing and genuinely advancing ticket-publication epoch/revision.

An invitation that is stale, expired, from a forked/substituted authority,
addressed to another Account, signed by an inactive Device, or below the old
high-water mark fails closed. A different replacement Device may have its own
publication epoch, so cross-Device safety is anchored in the Root-signed
authority revision rather than comparing unrelated per-Device epochs.

## Explicit confirmation and artifact

Preparation generates a fresh 32-byte nonce and a validity window no longer
than 15 minutes and never longer than the invitation. It derives a short exact
`REENROLL` confirmation phrase from the complete plan ID. Signing requires both
the exact phrase and the explicit `UserOrTestOperatorApproved` enum; neither a
release key, Account Root, remote peer nor network input can substitute for the
local Device signature.

The canonical bounded artifact exact-binds:

- both Account IDs and the local signing Device;
- exact digests and revisions of both Root-signed device lists;
- old peer Device, quarantine record ID/sequence/reason and predecessor
  record/floor;
- replacement peer Device, invitation ID, required capability ID and
  publication epoch;
- preparation/expiry times and the fresh nonce.

It contains no connection ticket body, endpoint, address, relay URL, message,
ciphertext, filesystem path, Account Root secret or Device secret. Context is
re-verified before later use, so a changed quarantine head, authority list or
invitation invalidates the artifact.

## No apply authority in M1.2.10

`kilogram-peer-reenrollment` is a pure offline state-machine crate. It has no
redb writer, runtime IPC, transport, CLI, desktop or network dependency and
offers no apply or clear API. Creating or verifying a tombstone leaves the
capability database unchanged. The old quarantined chain remains evidence and
ordinary network input still cannot lower its floor.

A later milestone may define an atomic durable acceptance receipt and a new
peer-key floor, but only while retaining the old quarantine plus this exact
Device-signed tombstone. That future step must not reinterpret this planning
artifact as self-applying authority.

## Verification

Stable-path Windows harnesses cover exact quarantine extraction without
mutation, explicit approval and phrase confirmation, wrong signer, tampering,
substituted invitation, expiry, same-Device stale/regressed publication state,
missing/non-quarantined state and canonical encode/decode. The fail-closed
boundary verifier mutates scope, prerequisites and authority claims.

Aggregate readiness passes 136/136 together with rustfmt and workspace-wide
locked/offline strict Clippy. No release build, ZIP, tag, automatic GitHub
build, second-host run or network test is required for this offline milestone.

## Non-claims

M1.2.10 does not apply re-enrollment, clear quarantine, retry delivery, change
contacts, prove global authority freshness, resist coordinated whole-store
rollback without an external witness, solve unknown first-contact extension
stripping, authorize public canonical rollout, or constitute an independent
security review.
