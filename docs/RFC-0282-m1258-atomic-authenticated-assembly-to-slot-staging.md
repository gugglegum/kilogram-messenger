# RFC-0282: Atomic authenticated-assembly-to-slot staging (M1.2.58)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.57 removed caller ownership of a ready launch authorization by creating it
inside the process-owned slot. The caller still had to construct and temporarily
own the complete M1.2.53 issuer before passing it to that slot. Although the
issuer was one-shot, this intermediate production seam could later become a
place to retain, route, or replay ready launch authority.

## Decision

The exact authenticated signed Plan, coordinator, local signer, state binding,
and ordered live memberships are now assembled into one opaque crate-private
`RecipientImportGuidedRecoveryAssemblyStagingInput`. Its fields are private and
the value implements neither Clone, Debug, Serialize, nor Deserialize. It does
not expose loose path, account, device, signer, descriptor, request,
coordinator, or membership fragments.

`RecipientImportGuidedRecoveryLaunchSlot::stage_from_authenticated_assembly`
is the sole production transition. It accepts the opaque staging input by
value, checks existing slot custody, creates the one-shot issuer inside the
slot, immediately seals that issuer into the M1.2.54 authorization, validates
clock and expiry, and only then retains the grant. Production calling code
never owns a ready issuer or authorization.

The old production issuer-construction seam is removed. A constructor remains
only under explicit `#[cfg(test)]` so the already verified M1.2.53 and M1.2.54
layer invariants can still be tested independently.

## Fail-closed ownership

Invalid lifetime, clock regression, expiry, binding failure, and validation
failure consume the staging input, issuer, and any partially created grant.
Pending or installed-host replacement consumes the incoming staging input
without creating another issuer and leaves existing custody unchanged. No
failure returns authority, permits replay, silently replaces state, or evicts
the current grant or host.

Success returns only the existing safe slot-state projection. The staging
input, issuer, authorization, installation owner, host, authenticated session,
handoff, identity, signer, request, coordinator, and membership remain
unavailable. Input, issuer, authorization, and slot remain process-local,
non-cloneable, non-serializable, and unrecoverable after restart.

## Dormant product boundary

This stage adds no runtime IPC command, CLI command, desktop control, timer,
background start, filesystem discovery, peer wire, runtime wire, or network
activity. Ordinary startup neither creates the slot nor stages an assembly.
The public sync default remains `legacy-only`.

## Verification

Two stable-path real-storage fixtures cover atomic successful staging, safe
preview/cancel, invalid lifetime, pending replacement, and preservation of
existing custody. They passed 2/2 in 106.03 seconds. The combined guided-
recovery regression passed 22/22 in 2147.89 seconds.

The dedicated boundary verifier rejects 18 negative mutations. Accepted-tag
M1 readiness passed 184/184 gates alongside rustfmt and workspace-wide locked,
offline, all-target, all-feature strict Clippy.
