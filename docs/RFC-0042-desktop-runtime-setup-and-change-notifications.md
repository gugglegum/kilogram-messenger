# RFC-0042: Desktop runtime setup and actor change notifications

Status: implemented in M0.9.15 (2026-09-03).

## 1. Goal

M0.9.15 removes the mandatory `runtime-profile-create` CLI step for an already
enrolled device and replaces the desktop client's two-second chat polling with
authenticated actor-owned change notifications.

This is runtime setup, not account creation or device enrollment. The GUI still
does not receive a seed, Account Root secret, device private key, vault key,
ratchet state or direct storage access.

## 2. Editable secret-free launch profile

The desktop launch panel can load, edit and atomically save launch-profile v1.
It exposes only public configuration and paths:

- enrolled device state directory;
- signed device-list and peer prekey-pool inputs;
- allowed requester Account ID and route/relay policy;
- public runtime ticket and machine-local IPC descriptor outputs;
- bounded relay wait, runtime poll, retry and automatic-sync intervals.

Existing input paths are canonicalized and must have the expected file or
directory type. Output parents are created and resolved before persistence.
Shared `RuntimeLaunchProfile` validation now enforces the same interval bounds
as the runtime CLI. Saving is allowed only while disconnected and without a
desktop-owned runtime process, then uses same-directory temporary persistence
and atomic replacement. The profile and IPC descriptor remain outside
protected state.

The original no-clobber `write_new` operation remains available to bootstrap
scripts. `write_replace` is the explicit editor operation; it never changes
device state or authority.

## 3. IPC v4 change revision

IPC v4 adds:

```text
WaitForChange { after_revision, timeout_milliseconds }
ChangeState { revision, changed }
```

The revision is an in-memory monotonic counter scoped to one runtime instance.
It is not an event sequence, consensus clock or durable security value. A new
signed descriptor/token on restart creates a new subscription lifetime; the
GUI always performs a full initial snapshot after `Ping`.

Change waits are authenticated and bounded to at most 25 seconds. They are
served by the loopback connection task from a Tokio watch channel and never
enter the bounded actor MPSC queue. Consequently a long-polling GUI cannot
block contact import, queue mutation, reads, shutdown or P2P work. A timeout
rechecks the current revision before returning, so a concurrent publication is
not reported as `changed=false`.

## 4. Publication boundary

The runtime publishes a revision only after an operation that may change a GUI
snapshot:

- a new contact or queue record committed by the actor;
- delivery or retry state committed by an outbound tick;
- an automatic synchronization attempt;
- a successfully completed inbound application session.

Idempotent contact/queue replay does not publish a new revision. Publication is
only a wake-up hint after the authoritative operation; clients must fetch fresh
actor-owned snapshots and must not infer domain state from the revision value.

## 5. Desktop subscription

A dedicated desktop worker maintains one authenticated long poll independently
of the ordinary sequential request worker. It tracks the last observed
revision, retries transient connection failure with a bounded delay and emits a
coalescible local wake-up. On a wake-up the GUI runs the existing serialized
pipeline:

1. conversation summaries;
2. selected conversation history;
3. outbox status.

If another GUI operation is active, the wake-up remains pending until the
single-operation boundary is free. Manual outbox refresh remains available,
but the unconditional two-second polling loop is removed.

## 6. Security and lifecycle

- Runtime remains the only cryptographic authority and state reader/writer.
- The profile editor cannot authorize a peer; runtime revalidates every signed
  authority, membership, prekey and contact input when it starts or imports.
- The IPC bearer descriptor remains private machine-local state and must not be
  placed in a synchronized folder.
- Closing or stopping a desktop-owned runtime ends its subscription; no service,
  Scheduled Task or autostart registration is introduced.
- Same-user malware and OS peer-credential hardening remain outside M0.

## 7. Verification

- profile replacement and GUI-draft tests cover canonical enrolled-device
  inputs, shared validation, atomic save/load and no-clobber compatibility;
- IPC tests prove a published revision wakes a waiter, a timeout reports no
  false change and `WaitForChange` never reaches the actor queue;
- desktop integration proves its change worker observes the revision without
  actor dispatch;
- rustfmt, strict workspace Clippy, all workspace tests and a release build are
  required before the milestone commit.

## 8. Deferred work

First-run account creation, secure Account Root/seed recovery, device enrollment
and peer ceremony still use their existing explicit tools. Those form the next
desktop boundary. Optional background/autostart mode, OS IPC peer credentials,
macOS/Linux/mobile key providers, wide-area discovery/gossip/mailbox and group
governance remain separate stages.
