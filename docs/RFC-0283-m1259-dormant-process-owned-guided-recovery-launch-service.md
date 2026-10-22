# RFC-0283: Dormant process-owned guided-recovery launch service (M1.2.59)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.58 made authenticated assembly staging atomic and removed caller ownership
of a ready issuer and authorization. The individual launch-slot methods still
formed the prospective integration surface, however. A future product caller
could easily route staging, approval, cancellation, and installed-host commands
through different owners or accidentally retain a raw slot alongside another
service abstraction.

## Decision

`RecipientImportGuidedRecoveryLaunchService` is the single dormant facade for
one complete guided-recovery launch lifecycle. Each service owns exactly one
private `RecipientImportGuidedRecoveryLaunchSlot`; there is no accessor,
replacement operation, extraction operation, second-slot constructor, Clone,
Debug, Default, Serialize, Deserialize, persistence, or restart recovery.

Construction and opaque assembly staging are crate-private. Staging accepts the
M1.2.58 assembly input by value and forwards it immediately to the owned slot.
The service routes safe state and preview, typed approval and pending
cancellation, and the existing typed host prepare, confirm, apply, cancel, and
refresh commands. Its results contain only the existing safe slot state,
launch preview, local UX view, and typed errors.

The facade never accepts loose path, account, device, signer, descriptor,
request, coordinator, or membership fragments. It never returns the raw slot,
host, issuer, authorization, installation owner, authenticated session,
handoff, assembly, identity, signer, or membership authority.

## Fail-closed custody

All ownership transitions remain governed by the already verified slot and
host rules. Invalid staging consumes incoming assembly authority. Pending and
installed-host replacement attempts preserve existing custody and consume the
incoming value. Explicit pending cancellation destroys the authorization.
Approval takes pending custody before fallible phrase, expiry, issuer, or host
work, so a failed approval cannot be retried. A failed installed-host command
keeps the same host owned by the service.

The service error is a typed wrapper around the existing slot error and carries
no rejected authority. No failure returns authority or silently replaces or
evicts current custody.

## Dormant product boundary

Ordinary runtime startup does not create the service or stage an assembly.
There is no runtime IPC command, CLI command, desktop control, timer,
background start, filesystem discovery, peer-wire change, runtime-wire change,
or network activity. The public sync default remains `legacy-only`.

## Verification

Two stable-path real-storage fixtures cover one complete facade-routed
lifecycle and fail-closed custody across invalid staging, pending cancellation,
approval mismatch, replacement attempts, and installed-host command failure.
They passed 2/2 in 244.59 seconds. The combined guided-recovery regression
passed 24/24 in 2326.12 seconds.

The dedicated boundary verifier rejects 18 negative mutations. Accepted-tag
M1 readiness passed 185/185 gates alongside rustfmt and workspace-wide locked,
offline, all-target, all-feature strict Clippy.
