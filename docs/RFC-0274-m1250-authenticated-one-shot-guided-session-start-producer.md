# RFC-0274: Authenticated one-shot guided-session start producer (M1.2.50)

Status: accepted as a dormant process-local authority transition. It is not
registered with desktop, runtime IPC, CLI, timers, background work, or product
networking.

## 1. Sealed owner hand-off

The already authenticated recovery owner seals its complete
`AuthenticatedGuidedRecipientImportSession` into an
`AuthenticatedGuidedRecipientImportSessionStartHandoff`. The hand-off retains
the exact state, Account, Device, signed descriptor, Plan request, coordinator,
ordered approved-source references, and live memberships inside private Rust
fields. It has no public constructor and no Clone, Debug, Serialize, or
Deserialize implementation. Loose paths, signers, requests, coordinators, or
membership snapshots therefore cannot be supplied to the producer.

The hand-off is valid for at most 300 seconds. Explicit cancellation destroys
its authority. It is not durable and cannot be restored after process restart.

## 2. Consume before validation

`RecipientImportGuidedSessionStartProducer` borrows the exact controller that
will consume the resulting command. It takes authority out of the hand-off
before checking time, requested session lifetime, controller binding, or any
durable recovery state. Consequently success, expiry, clock mismatch, invalid
lifetime, foreign binding, stale recovery state, and an already active session
all leave the hand-off consumed. Replaying the same value cannot produce a
second start command.

The requested UX session must fit entirely inside the remaining hand-off
lifetime and is independently capped at 300 seconds.

## 3. Exact authenticated preflight

Before releasing a `RecipientImportGuidedSessionLocalUxStart`, the controller
revalidates state-directory, Account, Device, descriptor signature, Plan
request, coordinator plan and sequence, ordered membership set, and every live
membership digest. It then reruns the canonical read adapter against the
current active intent, approved sources, inbox, import store, and coordinator.
Only a fresh `Planned` result while the controller is idle and its consumption
actor is unattached can produce the command.

The controller repeats the same preflight when the local UX actually starts
the session. State changed between production and use therefore fails closed.

## 4. Verification and product boundary

Stable-name real recovery fixtures prove one successful one-shot hand-off,
replay rejection, end-to-end transition into the M1.2.49 Planned view, expiry,
explicit cancellation, an overlong session, foreign state/Account/Device,
active-session rejection, and restart to Idle. A failed production attempt
never leaves reusable authority.

The producer opens no network path and adds no filesystem discovery. Ordinary
startup still owns neither a hand-off nor a start command. CLI, desktop,
runtime IPC, peer wire, timers, and background workers are unchanged, and the
public sync default remains `legacy-only`.

The two producer fixtures passed 2/2, and the stable-name combined guided
session regression passed 6/6. The dedicated boundary rejected all 17
weakening mutations, while the accepted-tag readiness inventory passed all
176 gates without starting test executables or product networking.

M1.2.51 implemented the process owner in RFC-0275. The new host retains at most
one sealed hand-off, consumes the resulting start command internally, and
exposes only typed local-UX commands plus the privacy-safe view.
