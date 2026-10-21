# RFC-0275: Dormant process-owned guided-recovery host (M1.2.51)

Status: accepted as a dormant process-local composition boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, or product
networking.

## 1. One owner and one ceremony

`RecipientImportGuidedRecoveryHost` owns the M1.2.48 controller and at most one
pending M1.2.50 hand-off. The host has no Clone, Debug, Serialize, or
Deserialize implementation. A newly constructed host is empty even when its
state directory contains durable recovery records; pending and active authority
are intentionally process-local and are never reconstructed after restart.

The authenticated in-process recovery owner installs authority through a
crate-private method. The host repeats the state-directory, Account, Device,
signed Plan, coordinator, membership, active-intent, and approved-source
preflight before creating the sealed hand-off internally. A second pending
authority, replacement attempt, replay while active, or foreign binding is
rejected. Incoming authority is dropped, while an existing non-expired pending
hand-off or active ceremony is not displaced.

## 2. Safe local UX surface

The public host surface consists only of typed `prepare`, `confirm`, `apply`,
`cancel`, and `refresh` commands. It returns the M1.2.49 privacy-safe local view.
No public method accepts or returns a hand-off, local-UX start command,
controller, state path after construction, signer after construction,
descriptor, coordinator, membership reference, membership snapshot, request
ID, nonce, or signing material.

`prepare` takes the pending hand-off out of the host before invoking the
M1.2.50 one-shot producer. The resulting start command is immediately consumed
inside the host by the local-UX adapter. Neither value crosses the host
boundary. Success and every failed prepare therefore leave no pending authority.

## 3. Cleanup and stale-command behavior

Pending cancellation is a typed command distinct from exact active-session
cancellation. A stale pending cancel cannot cancel an active session, and an
active-session cancel cannot discard a pending hand-off. Pending expiry is
purged before public refresh/cancel and before authenticated installation;
active expiry remains owned by the controller. M1.2.49 exact session/phase
checks continue to reject stale and double confirm/apply without repeating
durable work.

Dropping the host destroys all pending authority. Reconstructing it from the
same state directory starts Idle with no pending hand-off and no attached
consumption context.

## 4. Verification and product boundary

Stable-name real-storage fixtures prove one complete
prepare/confirm/apply/cancel lifecycle, preservation of the first pending
hand-off against replacement, preservation of a live session against replay,
foreign state/Account/Device rejection, failed-prepare cleanup, pending cancel,
pending expiry, and empty restart. The host opens no network path.

Ordinary startup still constructs no host and installs no authority. CLI,
desktop, runtime IPC, peer wire, timers, background workers, and filesystem
discovery are unchanged. The public sync default remains `legacy-only`.

The two host fixtures passed 2/2, and the stable-name combined guided-recovery
regression passed 8/8. The dedicated boundary rejected all 18 weakening
mutations, while the accepted-tag readiness inventory passed all 177 gates
without starting additional test executables or product networking.

M1.2.52 strengthened this boundary in RFC-0276. Host construction and
authenticated installation are private, and a consuming full-assembly owner is
now the only installation path.
