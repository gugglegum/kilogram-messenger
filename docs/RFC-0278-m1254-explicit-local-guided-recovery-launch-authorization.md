# RFC-0278: Explicit local guided-recovery launch authorization (M1.2.54)

Status: accepted as a dormant process-local authority boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, filesystem
discovery, or product networking.

## 1. One issuer, one local decision

`RecipientImportGuidedRecoveryLaunchAuthorization` owns exactly one M1.2.53
authenticated assembly issuer. It has no public constructor: only an already
authenticated in-process issuer can enter through the crate-private seam. The
authorization implements neither Clone, Debug, Serialize, nor Deserialize and
cannot survive a process restart.

The authorization is short-lived, with a hard maximum lifetime of 300 seconds.
It exposes only a safe preview: membership count, expiry, and a separately
domain-bound confirmation phrase. The preview contains no filesystem path,
Account or Device ID, descriptor, signer, coordinator, membership snapshot,
signed request, nonce, or authority object. The phrase commits to the exact
signed assembly plus the authorization timestamps without exposing those
inputs.

## 2. Consume before approval

The public `approve(self, typed_approval, now)` transition destructures and
consumes the authorization before checking clock, expiry, phrase, or downstream
assembly validation. Exact typed confirmation is required. Success emits only
one `RecipientImportGuidedRecoveryHostInstallationOwner`; it never returns the
issuer, raw session, host, hand-off, controller, or membership data.

Explicit `cancel(self)` also consumes the retained issuer. Invalid lifetime,
clock regression, expiry, phrase mismatch, canonical issuer-validation failure,
success, and cancellation all leave no reusable grant. This makes every result
fail closed and non-replayable.

## 3. Dormant product boundary

Ordinary startup creates or approves no launch authorization. There is no
runtime IPC command, CLI command, desktop control, timer, background worker,
filesystem-discovery path, or network registration. Peer wire and runtime IPC
wire are unchanged, and the public sync default remains `legacy-only`.

M1.2.54 does not yet add a process-wide slot. The authorization itself cannot
replace its retained issuer, while the existing host still refuses replacement
of pending or active custody. A following stage will own at most one pending
authorization in one process-local slot and make the pre-install replacement
rule explicit.

## 4. Verification

Stable-name real-storage fixtures cover exact typed approval through the
installation owner and host, plus invalid lifetime, clock regression, expiry,
phrase mismatch, explicit cancellation, and downstream issuer failure. Source
and manifest mutation checks protect constructor privacy, consume-by-value
semantics, preview minimization, output narrowing, dormant integration, and the
unchanged product default.

The two focused authorization fixtures passed 2/2 in 119.64 seconds. The
combined M1.2.47–M1.2.54 guided-recovery regression passed 14/14 in 1491.09
seconds. Accepted-tag readiness passed all 180/180 gates with rustfmt and
workspace-wide offline strict Clippy.
