# RFC-0041: Desktop contact onboarding and runtime lifecycle

Status: implemented in M0.9.14 (2026-09-03).

## 1. Goal

M0.9.14 lets the desktop client import an already-authorized private-chat
contact and own the foreground lifetime of the messaging runtime. The GUI must
not gain direct access to protected state, device secrets, ratchets or storage
repositories.

This is not first-contact key discovery. Conversation membership and the peer
account remain explicit trust inputs created by the existing authority
lifecycle.

## 2. IPC v3

The local descriptor/signature domain is advanced to v3. Two authenticated
commands are added:

- `AddContact { conversation, expected_peer_account_id, descriptor_file }`;
- `Shutdown`.

`AddContact` is dispatched through the same bounded loopback connection and
single runtime actor as queue/read operations. Under the short state lock and
vault dual-write guard, the actor:

1. loads the local device certificate, current authority and exact conversation
   membership;
2. requires both the local and expected peer accounts to be participants;
3. canonicalizes the public peer ticket path and rejects a path inside
   protected state;
4. verifies the ticket signature, peer Account ID, certified peer Device ID,
   route policy and authorization of the local account/device;
5. observes the signed peer prekey directory, pins the peer authority snapshot
   and appends the device-signed runtime contact.

Exact repeated import returns `inserted=false`. A conflicting immutable contact
record fails closed. The GUI never parses or approves the ticket by itself.

`Shutdown` replies with `ShutdownAccepted`, gives the loopback response a short
delivery window, then exits the normal runtime loop. The runtime removes only
its own compare-before-delete IPC descriptor and closes Iroh normally.

## 3. Secret-free launch profile

`runtime-profile-create` writes a versioned JSON profile with no-clobber
semantics. It contains only absolute paths and public runtime settings:

- state directory and signed authority/prekey input paths;
- public ticket and private machine-local IPC output paths;
- allowed requester Account ID, route/relay selection;
- bounded poll, retry and automatic-sync intervals.

It contains no seed, device private key, vault key, IPC bearer token or message
plaintext. The profile itself and IPC descriptor must be outside protected
state. Existing input paths are canonicalized, output parents are resolved, and
the same runtime option validation runs before the profile is persisted.

`runtime-from-profile` validates and loads this profile, then invokes the normal
runtime with unbounded foreground process limits. It does not create a second
runtime implementation.

## 4. Desktop lifecycle

The GUI accepts `--runtime-profile PATH`, `--runtime-exe PATH` and the existing
`--ipc-file PATH`. With a launch profile it can:

- start the sibling `kilogram-cli` as a hidden child process;
- derive the IPC descriptor path from the validated profile;
- retry authenticated `Ping` at a bounded interval for at most 60 seconds while
  Iroh and relay selection initialize;
- stop a connected runtime through authenticated `Shutdown`;
- request graceful shutdown for a desktop-owned runtime when the window closes,
  wait for a bounded interval, and use process termination only as crash
  fallback.

An externally started runtime can still be connected or stopped explicitly.
The GUI does not register a service, Scheduled Task or autostart entry.

The `+ Contact` form accepts a conversation label, expected peer Account ID and
public peer runtime ticket path. Drag-and-drop fills the peer ticket path while
the form is open. A successful actor response refreshes the signed contact list.

## 5. Security boundary

- Runtime remains the only state reader/writer and cryptographic authority.
- The launch profile is configuration, not authorization; every authority and
  contact invariant is rechecked by runtime.
- The IPC bearer descriptor remains private machine-local state and must not be
  put in a synchronized directory.
- Same-user malware remains outside the M0 guarantee. OS peer credentials and
  stronger process pinning remain future hardening.
- Runtime stdout/stderr are not piped into the GUI, preventing an undrained child
  pipe from blocking a long-lived messenger process.

## 6. Verification

- launch-profile tests cover bounded decode, no-clobber persistence, absolute
  authority paths and rejection inside protected state;
- a real runtime starts from a profile, publishes ticket/IPC descriptors,
  accepts authenticated shutdown and removes the IPC descriptor;
- the Alice/Bob runtime process test imports Bob through IPC before queueing and
  still converges to one text event plus acknowledgement;
- desktop adapter coverage exercises `Ping`, `AddContact`, queue, outbox,
  conversation list, history and `Shutdown` against a signed loopback server;
- rustfmt, strict workspace Clippy, all workspace tests and release build are
  required before the milestone commit.

## 7. Deferred work

The profile is still created by a bootstrap CLI command. First-run account and
device setup, GUI profile editing, push/change subscriptions, optional explicit
background/autostart mode, first-contact discovery, contact rotation and
multi-platform key providers remain separate stages.
