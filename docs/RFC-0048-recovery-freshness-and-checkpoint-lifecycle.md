# RFC-0048: Recovery freshness and checkpoint lifecycle

Status: M0.9.27 live runtime device-directory refresh implemented (2026-09-04).

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

## 5. Implemented M0.9.22 core

`kilogram-identity` now defines two bounded binary artifacts:

- `.karq`: an unsigned fresh request containing the exact Root-signed package,
  matching Root-signed witness, random 256-bit challenge, issue time and expiry;
- `.kara`: a device-signed approval binding Account ID, request/package IDs,
  authority revision, state-vector digest, exact recovery-roster digest,
  challenge, expiry, approver, approval time and previous local approval-head
  digest.

The request is valid for 10 minutes by default and at most 30 minutes. A
different challenge produces a different Request ID, so an approval from an old
attempt cannot be replayed. Decoders enforce magic, version, size and time
bounds before a claim is evaluated.

The package device-list revision must now equal the package authority revision,
not merely be older than it. Every approval is checked against that package's
current authority snapshot, so an entry from a stale pre-revocation list cannot
act as a current voter.

The one-shot helper exposes:

```text
kilogram-bootstrap account-recovery-quorum-request
kilogram-bootstrap account-recovery-quorum-approve
kilogram-bootstrap account-recovery-quorum-verify
```

Approval requires an initialized encrypted state vault. Under the exclusive
device-state lock it reads the certificate, authority, memberships and previous
approval head from DB-primary records. The candidate must contain the exact
local device certificate and dominate every locally known own-account authority
and membership head. Lower revisions, missing memberships, same-revision forks,
non-add-only membership updates, revoked devices and different certificates fail
closed.

Before a `.kara` file becomes visible, one trust transaction advances the local
high-water state and stores `recovery-approval/latest.approval` in DB-primary
state. Publication failure therefore cannot release an unrecorded signature; a
retry for the same Request ID returns the exact committed approval. The device
state lock serializes concurrent attempts. A later request chains the previous
approval ID.

Until recovery-policy epochs exist, the first committed approval head freezes
the exact roster digest for that device. A different roster is rejected even if
Root-signed. This is deliberately conservative: adding or revoking a recovery
voter needs the future joint old/new-majority transition before the product can
claim cross-roster fork safety.

The verifier rejects duplicate Device IDs and approvals bound to another
request, challenge, package or roster. It reports 0 approvals as
`artifact-integrity-only`, one sub-majority approval as
`single-current-device-observed`, and a strict majority as
`current-device-majority-observed`. `--require-majority` turns an insufficient
result into a hard failure. It always reports `cross_roster_fork_safety=false`.

Regression covers expiry/replay, duplicate approvals, insufficient quorum,
explicit offline fallback, DB-primary commit-before-publish and idempotent retry,
stale membership omission, same-revision membership fork and different-roster
rejection. Revocation is enforced both by candidate dominance and by checking
each voter against the package's exact current authority.

## 6. Implemented M0.9.23 transport and desktop ceremony

The one-shot helper now exposes two additional commands:

```text
kilogram-bootstrap account-recovery-quorum-listen
kilogram-bootstrap account-recovery-quorum-collect
```

An approver binds an Iroh endpoint, validates the exact `.karq` against its
DB-primary state, commits or reuses the local approval head, and only then
publishes a no-clobber `.kart` ticket. The ticket is signed by that current
device and binds the request/account/expiry, exact candidate certificate,
endpoint, bearer capability and `auto`, `direct-only`, or `relay-only` policy.
The listener releases the already committed `.kara` exactly once after a peer
presents the bound Request ID and bearer capability over the ticket endpoint;
an uncollected listener exits no later than the request expiry.

The transport gives the collector authenticated possession of the named current
device endpoint and an end-to-end encrypted QUIC path through LAN, hole-punched
Internet, or Iroh relay. Authentication is intentionally asymmetric: a
disaster-recovery client is not yet enrolled, so the `.kart` is an explicit
short-lived bearer consent. A stolen ticket can consume that one attempt and
observe privacy-sensitive public recovery metadata, but cannot forge another
device approval, change its request binding, obtain device/Root/vault secrets,
or satisfy a different quorum.

The collector rejects duplicate ticket Device IDs, validates every ticket
against the exact request roster, writes each response under its Device ID with
idempotent no-clobber semantics, then invokes the unchanged M0.9.22 verifier.
`--require-majority` remains a hard gate; a partial result is never relabelled as
majority.

The Windows recovery panel orchestrates request creation, current-device
listener, multi-ticket collection and verification of saved `.kara` files. It
shows the literal freshness claim and approval threshold and continues to show
`cross-roster fork safety: false`. Restore is gated by a majority for the exact
inspected package or an explicit reduced-assurance offline confirmation.

## 7. Implemented M0.9.24 recovery-policy epoch transition

Each participating device now retains a DB-primary policy anchor containing the
Account ID, epoch, exact recovery-roster digest and latest transition ID. Epoch
zero uses an all-zero predecessor and is derived only from the first locally
committed recovery approval head. A later epoch must name the preceding
transition ID; an old-roster signer rejects any request which does not extend
its exact local anchor.

A `.karpt` request binds the old and new Root-signed packages and witnesses,
`N -> N+1`, the predecessor transition ID, a fresh 256-bit challenge and a
bounded lifetime. The new package must be a monotonic authority/membership
successor, removed device certificates must have a corresponding revocation,
and the roster digest and authority revision must both advance.

Every `.karpa` signature binds that complete request and the signer's previous
transition-approval head. Before publication, the signer atomically stores the
head and advances applicable authority/membership high-water state in the
encrypted DB-primary trust repository. A device present in both rosters counts
once toward each threshold. A new-only device counts only toward the new
threshold and cannot replace old-roster consent. A different request for an
already signed epoch fails closed.

A permanent `.karpc` is canonical and valid only with distinct signatures
satisfying both strict majorities. Installation is allowed only on an active
new-roster device and atomically advances its policy anchor. Existing recovery
approval heads remain frozen unless the installed certificate's old digest
matches that head and its new digest matches the candidate package. This makes
legitimate enrollment/revocation possible without granting the Root key alone
the power to replace the recovery electorate.

`account-recovery-quorum-verify --policy-certificate-file` checks that the
candidate package extends the certified new package. It reports
`cross_roster_fork_safety=true` only when the ordinary exact-package strict
majority is also present. The claim assumes at least one honest member in every
required majority and durable signer anti-equivocation state; loss of an old
majority is intentionally not bypassed. For example, safe `2 -> 1` removal
requires both old voters, while `3 -> 2` can be authorized by the two retained
voters.

The current helper exposes:

```text
kilogram-bootstrap account-recovery-policy-transition-request
kilogram-bootstrap account-recovery-policy-transition-approve
kilogram-bootstrap account-recovery-policy-transition-certify
kilogram-bootstrap account-recovery-policy-transition-verify
kilogram-bootstrap account-recovery-policy-transition-install
```

## 8. Implemented M0.9.25 network collection and desktop activation

Every old/new-roster voter can now commit its transition approval and publish a
signed one-shot `.karpticket`. The ticket binds the exact Account and Request
IDs, request expiry, exact voter certificate, Iroh endpoint, 256-bit bearer
capability and `auto`, `direct-only` or `relay-only` policy. The DB-primary
transition-approval head is committed before ticket publication. A listener
then returns exactly one `.karpa` only after an authorized fetch presents the
matching Request ID and bearer.

The collector revalidates the ticket signature and exact old/new voter
membership, authenticates the Iroh endpoint, enforces the signed route policy,
and independently verifies the transported `.karpa`. Duplicate signer Device
IDs fail closed. Each approval is stored idempotently as
`{device_id}.karpa`, after which the unchanged joint-quorum verifier reports
old and new progress separately. Even a complete collection reports
`cross_roster_fork_safety=false`: the permanent claim begins only after
canonical `.karpc` certification.

The additional helper commands are:

```text
kilogram-bootstrap account-recovery-policy-transition-listen
kilogram-bootstrap account-recovery-policy-transition-collect
```

The Windows device-roster panel now orchestrates `.karpt` creation, one-shot
approval listeners, multi-ticket collection, certification and per-device
installation. Its status display deliberately separates three facts:

1. whether the Account Root roster mutation has happened;
2. whether a joint-majority recovery-policy certificate has been installed on
   this device;
3. whether message history has been recovered through the separate multi-source
   recovery workflow.

The GUI never transports Root/device private keys and does not infer policy or
history completion from a successful device-link operation.

## 9. Implemented M0.9.26 first-class device removal

`kilogram-bootstrap device-remove` accepts one full Device ID and an exact,
independently witnessed recovery checkpoint from before the operation. The
Root writer verifies that this checkpoint is still byte-exact current, refuses
an unknown device or the last active device, writes the permanent revocation,
and publishes the complete remaining device list at revision `N+1`.

The two Root files are a logical fail-closed transaction. A crash after the
revocation but before the list publication makes recovery export impossible
because the revisions differ. Repeating the same removal reuses the existing
revocation and repairs the list; it never allocates a second authority
sequence. Exact-removal validation admits no unrelated certificate,
membership, or revocation change between the supplied before package and the
new after package.

The helper idempotently publishes four public outputs outside the Root:

1. the Root-signed device revocation;
2. the refreshed complete active-device list;
3. the exact after-removal recovery package;
4. its exact independently retainable witness.

It then records the new package/witness as the local current checkpoint. The
Windows desktop requires the operator to type the full Device ID twice, feeds
the exact before/after package and witness paths into the M0.9.25
joint-majority transition workflow, and points the runtime-profile draft at the
new public device list. It deliberately exposes five independent lifecycle
states: Root removal is complete; recovery-policy activation is required;
runtime peer-directory refresh is required; ratchet/session retirement is
required; existing history copies remain readable.

## 10. Implemented M0.9.27 runtime activation

The installed revocation can now be applied to a long-lived runtime through an
authenticated typed IPC operation. The runtime accepts only an idempotent replay
or a Root-signed monotonic removal-only subset which retains its own exact
certificate. One vault-primary transaction installs the authority high-water
mark and removes revoked-device ratchet sessions and prekey observations; the
current public runtime ticket is then atomically replaced without changing the
Endpoint ID.

Senders dynamically reading that refreshed ticket retire the revoked recipient
state before new fanout materialization, so new recipient tables exclude the
device. Already signed events are append-only and are explicitly reported as
not rewritten; history already held by the removed device remains readable.
The complete contract and failure boundaries are specified in
[`RFC-0049-live-runtime-device-directory-refresh.md`](RFC-0049-live-runtime-device-directory-refresh.md).
