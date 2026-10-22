# RFC-0286: Host installation-owner public surface retirement (M1.2.62)

Status: implemented and locally verified on 2026-10-22.

## Problem

M1.2.61 made the installed recovery host crate-private, but the one-shot
`RecipientImportGuidedRecoveryHostInstallationOwner` that yielded that host
was still a public runtime type. Its constructors and `install` operation were
already internal, yet external code could name the raw custody type and the
issuer and authorization transitions still publicly declared it as a return
value.

That transitional API contradicted the process-owned slot boundary: approval
is supposed to install a host atomically inside the slot and expose only safe
state, not return intermediate installation authority to a caller.

## Decision

The installation owner is now a crate-private implementation detail and is no
longer publicly re-exported by the runtime crate. The copied-module execution
harness re-exports it only as `pub(crate)` so existing one-shot and failure
fixtures can continue to exercise the real source without reopening a product
API.

Every transition that returns the owner is equally narrow. Assembly-issuer
`issue`, launch-authorization `approve`, and owner `install` are crate-private
and consume `self`. The crate-private launch slot remains the only production
path that approves an authorization, receives the owner, installs the raw
host, and retains that host. No failure returns the owner, authorization,
issuer, or host to its caller.

The public safe vocabulary is unchanged: the launch-service facade, safe
preview and slot state, typed launch and host commands, typed errors, and the
privacy-safe local UX view. This is a visibility retirement only; custody,
validation, failure, and wire semantics are unchanged.

## Fail-closed boundary

The machine-readable contract and dedicated verifier reject public owner
visibility or re-export, public `issue`/`approve`/`install`, a second runtime
owner path, copied-test public re-export, and CLI/product exposure. They also
pin the public service and safe command vocabulary and retain the predecessor
one-shot regression fixtures.

## Product boundary

No runtime IPC, CLI, desktop, timer, background task, filesystem discovery, or
network path is registered. Ordinary startup creates neither service, owner,
nor host. Peer and runtime IPC wire formats are unchanged, and the public sync
default remains `legacy-only`.

## Verification

- the dedicated boundary rejects 18/18 weakening mutations;
- the unchanged combined guided-recovery regression passes 24/24 through the
  stable-name Windows harness;
- accepted-tag readiness passes 188/188 gates with rustfmt and locked/offline
  strict Clippy;
- no release build, archive, tag, automatic GitHub build, product network, or
  second host is used.
