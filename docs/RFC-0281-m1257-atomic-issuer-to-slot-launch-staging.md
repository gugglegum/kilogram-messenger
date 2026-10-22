# RFC-0281: Atomic issuer-to-slot launch staging (M1.2.57)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.53 created a one-shot authenticated assembly issuer, M1.2.54 wrapped it
in a short-lived launch authorization, and M1.2.55 placed that authorization
under process-owned slot custody. The boundaries were individually one-shot,
but their composition still let crate-internal calling code temporarily own a
complete, ready launch authorization between construction and slot admission.

That intermediate ownership was unnecessary. A later product integration
must not be able to retain, replay, replace, or return a ready grant merely
because it performs the hand-off between two security layers.

## Decision

`RecipientImportGuidedRecoveryLaunchSlot` now exposes one crate-private
`stage_from_authenticated_issuer` transition. It accepts the M1.2.53 issuer by
value together with bounded authorization timing, constructs the M1.2.54 authorization inside the empty slot,
validates clock and expiry, and only then retains it.

The former slot seam accepting a prebuilt
`RecipientImportGuidedRecoveryLaunchAuthorization` has been removed. The
caller can no longer hold a complete launch grant between issuer and slot.
Success returns only `PendingAuthorization`, the existing safe slot-state
projection. The issuer, authorization, installation owner, host, authenticated
session, handoff, identity and membership remain unavailable.

## Fail-closed ownership

The transition owns the incoming issuer before checking any fallible path.
Invalid lifetime or launch-binding failure consumes it during authorization
construction. Clock regression or expiry consumes the newly constructed grant
before retention. If the slot already owns a pending authorization or an
installed host, the incoming issuer is dropped and the existing custody is
left unchanged. No rejection returns authority, silently replaces state, or
evicts the current grant or host.

The slot and the retained values remain non-cloneable and non-serializable.
They are not persisted and cannot be recovered after restart.

## Dormant product boundary

This change adds no runtime IPC command, CLI command, desktop control, timer,
background start, filesystem discovery, peer wire, runtime wire, or network
activity. Ordinary startup neither creates the slot nor stages an issuer. The
public sync default remains `legacy-only`.

## Verification

Two stable-path real-storage fixtures cover successful internal construction,
safe preview/cancel, invalid lifetime, clock regression, expiry, pending
replacement, installed-host replacement, and preservation of existing
custody. They passed 2/2 in 123.30 seconds. The combined M1.2.47-M1.2.57
guided-recovery regression passed 20/20 in 2116.74 seconds.

The dedicated boundary verifier rejects 18 negative mutations. Accepted-tag
M1 readiness passed 183/183 gates alongside rustfmt and workspace-wide locked,
offline, all-target, all-feature strict Clippy.
