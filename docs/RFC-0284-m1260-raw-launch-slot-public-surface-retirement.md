# RFC-0284: Raw launch-slot public surface retirement (M1.2.60)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.59 introduced the single dormant process-owned launch service, but the
underlying `RecipientImportGuidedRecoveryLaunchSlot` type was still publicly
re-exported by the runtime crate. Its constructors were already crate-private,
yet an external caller could still name the implementation owner in its own
API and keep depending on a custody layer that the service is meant to hide.

That unnecessary public seam also made it easier for a later integration to
grow a second owner, raw accessor, or replacement path around the service.

## Decision

The raw launch slot is now a crate-private implementation detail. The runtime
crate no longer publicly re-exports the type. The M1.2.59 service is the only production owner
and constructor user, and it continues to own exactly one
private slot without an accessor, replacement operation, extraction operation,
Clone, Debug, Default, serde codec, persistence, or restart recovery.

The safe vocabulary required by external facade users remains public:
`RecipientImportGuidedRecoveryLaunchSlotState`, typed approve and cancel
commands, the typed slot error, the safe launch preview, service errors, and
the service facade itself. This is an API visibility retirement only; command
semantics, custody transitions, error semantics, and wire representation do
not change.

## Test-only predecessor boundary

The execution crate still copies the predecessor module graph for real-storage
regression fixtures. Inside that copied-module test boundary the raw slot is
re-exported only as `pub(crate)`, while the safe command, state, and error types
remain public to the test module graph. Existing atomic staging, one-shot
approval, cancellation, host routing, and service facade fixtures remain
available without reopening production API.

## Fail-closed ownership boundary

Repository verification rejects a public raw struct, a public raw re-export,
a public service field or raw accessor, any second production owner, or a
public test-harness re-export. It also verifies that the safe facade vocabulary
was not accidentally privatized.

## Dormant product boundary

Ordinary startup still creates neither service nor slot. No runtime IPC, CLI,
desktop control, timer, background start, filesystem discovery, peer-wire,
runtime-wire, or network behavior is added. The public sync default remains
`legacy-only`.

## Verification

This retirement adds no behavioral test case. The unchanged combined
guided-recovery stable-path regression passes 24/24 fixtures in 2328.66
seconds. The dedicated
boundary verifier rejects 18/18 negative mutations and accepted-tag M1
readiness passes 186/186 gates with rustfmt and workspace-wide locked, offline,
all-target, all-feature strict Clippy.
