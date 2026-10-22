# RFC-0285: Raw installed-host public surface retirement (M1.2.61)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.60 retired the raw launch slot from the public runtime API, but the
installed `RecipientImportGuidedRecoveryHost` was still publicly re-exported.
Its constructor was private, yet external code could name the implementation
type in its own API and retain a dependency on custody that belongs behind the
crate-private slot and the public launch-service facade.

That remaining name also made a future second owner, raw accessor, extraction
operation, or replacement seam easier to introduce accidentally.

## Decision

The raw installed host is now a crate-private implementation detail. The
runtime crate no longer publicly re-exports it, and its one-shot installation
method is crate-private because it returns the raw host. Outside the host's own
definition module, the crate-private launch slot is the only production module
allowed to name, store, install, or mutably route commands to the host.

The safe vocabulary remains public: typed prepare and cancel commands, typed
host and service errors, the existing safe local UX view and commands, and the
launch-service facade. Existing `prepare`, `confirm`, `apply`, `cancel`, and
`refresh` routing stays behind the slot and service. This is an API visibility
retirement only; custody, command, error, and wire semantics do not change.

The transitional installation-owner type remains public for predecessor API
compatibility, but external code cannot invoke its crate-private installation
transition or obtain the raw host. Its remaining public surface is a separate
retirement target rather than a reason to reopen this implementation type.

## Test-only predecessor boundary

The execution crate still copies the predecessor module graph for real-storage
regression fixtures. Inside that copied-module test boundary the raw host is
re-exported only as `pub(crate)`. Existing host, installation-owner, launch
authorization, slot, router, staging, and service fixtures remain available
without reopening production API.

## Fail-closed ownership boundary

Repository verification rejects a public raw struct, public raw re-export,
public installation method, public slot field, host accessor, extraction or
replacement seam, second production owner, public copied-module re-export, or
direct product-surface use. It also verifies that safe typed commands, errors,
local UX projections, and service routing remain public.

## Dormant product boundary

Ordinary startup still creates neither service nor host. No runtime IPC, CLI,
desktop control, timer, background start, filesystem discovery, peer-wire,
runtime-wire, or network behavior is added. The public sync default remains
`legacy-only`.

## Verification

This retirement adds no behavioral test case. The unchanged combined
guided-recovery stable-path regression passes 24/24 fixtures in 2266.98
seconds. The dedicated boundary verifier rejects 18/18 negative mutations and
accepted-tag M1 readiness passes 187/187 gates with rustfmt and workspace-wide
locked, offline, all-target, all-feature strict Clippy.
