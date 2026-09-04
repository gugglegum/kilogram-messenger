# RFC-0048: Recovery freshness and checkpoint lifecycle

Status: M0.9.21 lifecycle implemented and quorum protocol accepted
(2026-09-04). Live quorum transport and approval artifacts are scheduled for
M0.9.22.

## 1. Problem and impossibility boundary

A Root-signed recovery package plus its Root-signed witness proves integrity and
exact agreement, but it cannot prove that the pair is the newest pair ever
issued. An attacker who can replace both files can replay an older matching
checkpoint. Adding another hash, signature, sequence, or hash-chain link inside
the same rollbackable storage does not change that result.

Freshness therefore requires at least one state source that the attacker cannot
roll back together with the artifacts:

- a live current device with authenticated local high-water state;
- a privacy-preserving external monotonic witness or transparency service;
- an OS/hardware monotonic counter with portable recovery semantics;
- or a human-controlled independent record whose freshness the user verifies.

Kilogram keeps the offline independent-witness recovery mode for availability,
labels its limitation, and chooses a live strict-majority current-device quorum
as the serverless stronger mode. A future external witness can implement the
same verifier interface when no device majority is available.

## 2. Implemented exact-export lifecycle

Every Account Root now has a local checkpoint lifecycle derived from its actual
current state. The last successful export is represented inside the Root by the
same bounded Root-signed witness that was published externally. The receipt
contains no phrase or private key.

Status is calculated under the Account Root authority lock by rebuilding the
canonical recovery package from the current authority snapshot, complete device
list, revocations, and all current membership heads, then comparing its exact
package ID and revision with the recorded witness:

- `current`: the last successful export is byte-exact for current Root state;
- `update-required`: no successful export is recorded, or authority/membership
  state has changed since it was recorded.

This catches membership changes even when the Account authority revision does
not change. An idempotent operation that leaves all captured state unchanged
does not make the export stale. A malformed, oversized, non-regular, incorrectly
signed, or cross-account local receipt fails closed rather than being silently
treated as current.

Export uses two phases:

1. capture one Root-locked package/witness pair and publish both external files
   with no-clobber semantics;
2. reacquire the Root lock, rebuild current state, require an exact match with
   the published package, then atomically record the local receipt.

If Root state changed between phases, recording fails and the helper removes the
new external pair. A mutation immediately after a successful recording simply
makes the next status calculation return `update-required`. A recovered Root is
created with the exact verified witness recorded and initially reports
`current`; enrolling its replacement device makes it stale again.

The one-shot helper exposes:

```text
kilogram-bootstrap account-recovery-status \
  --account-root-dir <CURRENT_ROOT>
```

Its bounded strict JSON reports current and recorded package IDs/revisions and
uses the explicit scope
`local-exact-export-receipt-not-global-freshness-proof`. The desktop recovery
panel exposes the same check and clears cached status whenever its Root path or
a Root-mutating device-link operation changes.

The local receipt is an operational lifecycle guard, not an independent
anti-rollback source. Rolling back the entire Root directory can roll it back as
well. The independently stored witness and live/external freshness evidence
remain required for disaster recovery.

## 3. Accepted live current-device quorum protocol

The stronger serverless ceremony uses a fresh 256-bit recovery-attempt challenge
created by the recovering client. The candidate package, exact witness, challenge,
expiry, and a protocol domain form one approval request. The challenge is never
reused; stored approvals from an earlier recovery attempt cannot satisfy it.

Each approving device must, while online and under its exclusive state lock:

1. read its identity and trust state from the authenticated DB-primary vault;
2. verify package and witness signatures and exact binding;
3. prove its certificate occurs in the candidate complete device list and is not
   revoked by the candidate authority snapshot;
4. require the candidate authority and every membership head to dominate all
   locally pinned heads, rejecting rollback and same-revision equivocation;
5. atomically persist a candidate-package approval head before releasing a
   device signature.

The signed approval content is bounded and domain-separated and contains:

- Account ID, recovery package ID, authority revision, and canonical state-vector
  digest;
- recovery-attempt challenge and expiry;
- approving Device ID and previous local approval-head digest;
- format version and device signature.

The collector accepts at most one approval per distinct, currently authorized,
non-revoked device. All approvals must bind the same exact active-voter roster
digest. The fixed M0 threshold is a strict majority of that roster:
`floor(active_devices / 2) + 1`. Thus one-device accounts require 1-of-1,
two-device accounts require 2-of-2, and three-device accounts require 2-of-3.
For one fixed roster, two conflicting packages cannot both obtain disjoint
strict majorities if an honest device persists its approval head and refuses
equivocation.

That intersection argument does **not** automatically hold across two different
device rosters. Production roster changes therefore require a recovery-policy
epoch transition approved by strict majorities of both the old and new roster
(joint consensus). Root signature alone may update messaging authority, but it
must not silently replace the recovery electorate. Until those joint transition
certificates are implemented, the M0 majority result is explicitly observation
of the exact candidate roster, not a globally fork-proof proof across roster
epochs.

Approval artifacts are public metadata but privacy-sensitive: they reveal an
Account ID, Device IDs, a recovery attempt, and timing. They contain no phrase,
Root key, device secret, vault key, ratchet state, or messages. Relay transport
may carry them end-to-end authenticated without learning those secrets.

## 4. Claims and fallback

The UI and verifier must expose one of these claims, never a generic “fresh”:

- `artifact-integrity-only`: exact package+witness, no live freshness evidence;
- `single-current-device-observed`: one current device approved this attempt;
- `current-device-majority-observed`: strict majority approved this attempt;
- `external-monotonic-witness-observed`: reserved for a future service/provider.

Majority observation is stronger than the offline pair but is still scoped to
the devices and local high-water states actually observed. It cannot defeat a
rollback or compromise of the entire majority, nor can it establish facts never
seen by any approver.

If too many devices are lost, strict majority recovery may be unavailable. The
user may deliberately fall back to phrase + independently retained package and
witness, with an explicit reduced-assurance warning. Recovery availability is
not silently converted into a false freshness claim. After any successful
restore/enrollment, a new package and independent witness must be exported and
the lifecycle must return to `current`.

## 5. M0.9.22 implementation requirements

The next stage will implement the bounded challenge/request/approval formats,
exact voter-roster binding, DB-primary monotonic approval head,
local/LAN-or-relay collection, strict-majority verification, and desktop claim
display. Recovery-policy epoch and joint roster transitions must be implemented
before claiming cross-roster fork safety. Tests must cover replay, duplicate and
revoked approvers, stale local heads, same-revision membership forks, concurrent
approval attempts, different-roster conflicts, insufficient quorum, and explicit
offline fallback.
