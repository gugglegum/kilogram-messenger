# RFC-0273: Dormant local-UX guided-session adapter (M1.2.49)

Status: accepted as a process-local, dormant presentation and command boundary.
It is not registered with desktop, runtime IPC, CLI, timers, background work,
or product networking.

## 1. Local presentation without authority export

M1.2.49 adds a short-lived facade that borrows the M1.2.48 process-owned
controller. It does not accept a state path, Device signer, coordinator,
membership reference, or membership snapshot. Start requires a dedicated
`RecipientImportGuidedSessionLocalUxStart` value containing the existing
authenticated process-local authority. That command has private fields, no
public constructor, and no Clone, Debug, Serialize, or Deserialize
implementation.

The public view is deliberately smaller than the controller state. It exposes
only phase, short-lived session ID, source counts, planned step count, imported
event count, deadline, the confirmation phrase while Planned, and the final
complete flag. It does not expose a state path, Account or Device identifier,
runtime descriptor, coordinator ID/object, membership reference/snapshot,
signing material, request ID, nonce, or authenticated start token. The view is
not serializable and is not a runtime IPC contract.

## 2. Typed commands and transition ownership

The facade has distinct start, confirm, apply, cancel, and refresh command
types. Confirm and apply accept only the short-lived session ID plus the exact
human confirmation phrase where required. The adapter generates cryptographic
request IDs internally and derives transition nonces with separate confirm and
apply domains; callers cannot reuse or substitute either value.

Every mutating command checks the exact live session ID before reaching the
controller. Confirm additionally requires Planned, and apply requires
Confirmed. A stale ID therefore cannot cancel or mutate a newer session. A
double confirm or double apply is rejected before durable work and preserves
the already reached phase.

## 3. Lost-response recovery

`refresh` purges expiry and projects the controller's current in-memory phase;
it performs no confirmation, import, consumption, or durable mutation. A UI
that loses a successful response must refresh instead of replaying the command.
The attached controller retains only the safe receipt fields needed to rebuild
the view, so recreating the facade in the same process returns the same result.

Refresh with an expected session ID rejects a different live session as stale.
If the expected session ended because of explicit failure, cancellation, or
expiry, refresh returns Idle. Process restart constructs a new controller and
therefore also returns Idle; durable ledgers never recreate ephemeral authority.

## 4. Fail-closed behavior and product boundary

An incorrect confirmation phrase reaches the canonical controller and destroys
that ephemeral session. Failure to generate a request ID or transition nonce
best-effort cancels the exact active session. Projection invariants reject any
view whose phase, counts, phrase, deadline, or completion flag disagree.

Real stable-name fixtures cover all four phases, facade recreation and exact
view recovery, stale IDs, incorrect confirmation, double confirm, double apply,
explicit detach, and restart. The combined M1.2.48-M1.2.49 regression passes
four fixtures without network or a second host. The accepted-tag readiness gate
passes 175/175 boundaries, rustfmt, and workspace-wide locked/offline strict
Clippy. Windows PowerShell treats Cargo stderr warnings as non-terminating and
uses the native exit code as the build authority.

Ordinary startup still creates neither start command nor session. No desktop,
CLI, runtime-IPC, timer, filesystem scan, or network entry point is added. Peer
wire and runtime IPC remain unchanged, and the public sync default remains
`legacy-only`. A later stage may create the start command only inside the
existing authenticated recovery owner; it must not reconstruct authority from
serialized fragments.
