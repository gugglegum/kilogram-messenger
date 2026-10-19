# RFC-0241: M1.2.17 guided desktop peer re-enrollment ceremony

Status: implemented and locally verified on 2026-10-20.

## Purpose

M1.2.16 completed the current-runtime mutation adapter, but the Windows client
could only observe peer re-enrollment. This stage exposes the already bounded
`status -> plan -> approve -> accept -> fresh start -> consumed` ceremony in
the desktop without moving Device or Account Root authority into the GUI.

The active runtime IPC remains version 33 with descriptor signature domain
`kilogram:runtime-ipc-descriptor:v14\0`. Connection-ticket and sync wire
formats, the public `legacy-only` default, and all network behavior are
unchanged.

## Authority boundary

The desktop loads and verifies the authenticated runtime descriptor for every
operation. Each nested request is bound to that descriptor and receives a new
request ID. Every response must be signed by the current Device and match the
exact request and descriptor before the UI installs it.

The GUI carries only an opaque contact ID and opaque content-addressed artifact
IDs. It may show the bounded replacement Device fingerprint already projected
by the signed response. It never receives a Device or Root secret, filesystem
path, transport ticket, endpoint bytes, raw Account/Device ID, reusable
capability, queue ID, message body, or history body.

## Explicit stages

1. The user enters an opaque contact ID and explicitly refreshes signed status.
2. `Ready` displays quarantine context and the bounded replacement summary.
   The user must select the exact staged invitation ID before `Plan` is
   enabled.
3. `ApprovalRequired` displays the exact `REENROLL ...` phrase. Approval needs
   both an exact typed phrase and a separate checkbox. Neither alone is
   authority.
4. `ApprovedPrepared` enables a distinct `Accept` action. Planning or approval
   never silently accepts the replacement.
5. `AcceptanceIncomplete` is rendered as failure/incomplete state and cannot
   enable restart. There is no clear-quarantine button or success message.
6. `AcceptedRestartRequired` is still not consumed. Hot apply is forbidden.
   A guided restart is available only when this desktop owns the connected
   runtime process.
7. The old process receives a clean shutdown request. A fresh process is
   launched from the existing signed launch profile, then queried over
   authenticated IPC. Success requires `Consumed` with the exact retained
   acceptance receipt and runtime transition IDs. A different transition or
   any other state stops the fresh child and fails closed.

The ceremony itself does not request message delivery, retry, recovery,
quarantine removal, or history rewrap. Normal runtime operation remains a
separate concern. Old readable evidence and quarantine records are not erased.

## Stale UI invalidation

Typed confirmation and the separate checkbox are cleared whenever any of the
following changes:

- opaque contact;
- signed response or exact plan;
- authenticated descriptor bytes;
- runtime connection/process generation;
- competing ceremony state.

A failed mutation discards the observed UI state and requires a fresh signed
status. A failed post-restart verification retains only the exact bounded
restart expectation so the user can retry a fresh process; editing the runtime
context discards that expectation.

## Verification

Pure state-machine tests prove exact invitation selection, two-part approval,
confirmation invalidation, rejection of incomplete acceptance, and exact
fresh-start transition verification. A real loopback `RuntimeIpcServer` test
drives all four authenticated desktop requests and rejects stage skipping or
outer read/mutation confusion.

The fail-closed boundary checks the manifest, UI state machine, IPC adapter,
guided restart lifecycle, absence of secrets/paths/clear/hot-apply authority,
five state tests, the authenticated loopback test, and negative manifest/source
mutations. Windows Rust tests run only from stable executable paths.

Aggregate readiness contains 143 closed gates. This stage does not run a
second-host or product-network test, create a release build, ZIP archive, tag,
or automatic GitHub build, and it does not claim public rollout eligibility or
independent security review.
