# RFC-0080: Controlled mailbox lifecycle field test (M0.9.58)

Status: implemented as a network-free test harness and fail-closed evidence
contract. The actual two-device run is deliberately pending an agreed field-test
window.

## 1. Goal

M0.9.55–M0.9.57 define mailbox capability lifecycle, deterministic restart
convergence and authenticated desktop control. Their automated coverage does not
prove that a real direct/relay connection can lose the recipient ACK at the
precise post-commit boundary and then converge after two process restarts.

M0.9.58 makes that experiment reproducible without weakening production code or
creating another executable. It covers:

- activation with a recipient-side ACK loss after durable apply;
- owner `activation-pending` state and exact idempotent retry after restart;
- rotation with an observable pending overlap and later convergence;
- one opaque mailbox fallback round trip with application commit before delete;
- revocation and final convergence;
- at least one direct and one relay lifecycle transfer;
- secret-free IPC status and a separate human check of the Windows projection.

## 2. Debug-only post-commit fault

The recipient runtime recognizes the process environment variable
`KILOGRAM_TEST_DROP_MAILBOX_CAPABILITY_ACK_ONCE=1` only in a debug build. Any
other value fails closed. A non-debug build rejects the variable before opening
the runtime endpoint.

The hook is one-shot and is consulted only for an authenticated
`MailboxCapabilityUpdatePush`. It runs after
`apply_runtime_mailbox_capability_update` has committed the signed update, but
before the recipient signs or writes its session-bound ACK. The normal
state-vault mirror then completes, the connection closes, and the runtime exits
cleanly with `runtime_stop_reason=debug-mailbox-capability-ack-drop`.

The hook cannot be set through IPC, the launch profile or the Windows GUI. It is
not present as an executable, protocol message, persistent state flag or release
feature. The field helper scopes the environment value to one foreground child
invocation and restores the caller's previous process environment afterwards.

## 3. Restart and replay invariant

After the controlled stop:

1. Bob has the exact generation-one peer update in durable state and no ACK was
   sent;
2. Alice retains the same update as `activation-pending`;
3. both runtimes restart without the fault variable;
4. Alice retries the same signed update ID rather than creating generation two;
5. Bob classifies the update as `AlreadyPresent`, signs a fresh ACK bound to the
   new authenticated session, and Alice commits it;
6. both status projections name the same binding/update and show generation one
   active.

This is a live counterpart to the pure convergence regression. It does not rely
on killing a process within an unrepeatably small timing window.

## 4. Evidence contract

`scripts/verify-kilogram-mailbox-field-evidence.ps1` consumes a versioned
manifest and fixed evidence filenames. It fails closed unless it can correlate
the exact Account/Device/conversation, binding IDs, update IDs and generations
across Alice and Bob.

It additionally requires:

- the debug fault's armed/applied/no-ACK/vault-mirrored/stop markers;
- `Inserted` before the loss and `AlreadyPresent` after restart;
- pending then acknowledged activation, rotation and revocation heads;
- direct and relay route evidence across lifecycle transfers;
- a store-signed `mailbox-stored` result, recipient
  `deleted-after-commit`, and durable receive/delete ledger counters;
- the store's `opaque-redb-v1` startup boundary and exact pinned public key;
- all existing blind-mailbox/runtime/lifecycle/convergence/desktop static
  boundary gates from the tested commit.

The verifier rejects a store log containing Account, Device, conversation,
event or message fields. That check proves only the supplied service output and
the code boundary; it is not a claim against a malicious host, reverse proxy,
kernel, network observer or traffic correlation.

`-SelfTest` creates temporary synthetic evidence, accepts the valid set, then
adds a forbidden metadata field and requires rejection. It does not bind a
network listener.

## 5. Operator helpers

`invoke-kilogram-mailbox-field-runtime.ps1` starts an existing debug CLI from an
existing secret-free runtime profile and writes a no-clobber phase log. Its
explicit switch arms the lost-ACK hook only for Bob.

`capture-kilogram-mailbox-field-status.ps1` asks the already-running actor for
secret-free mailbox status and accepts only the evidence filenames defined by
the procedure. Neither helper builds, optimizes or packages Rust artifacts.
Runtime profiles, IPC descriptors, state directories and secrets remain local
to each machine and must not be copied into the shared evidence directory.

## 6. GUI boundary

The machine verifier validates the same IPC projection consumed by
`kilogram-windows`; the existing integrated desktop test validates command and
response mapping. A field operator must still visually confirm the four
documented GUI states. A screenshot or checkbox would be an operator
attestation, not cryptographic proof, so the verifier does not mislabel it as
machine-verified evidence.

## 7. Non-goals

This milestone does not create a release/ZIP, run a network process, provision a
public service, hide IP addresses, prove proxy log deletion, test a malicious
store, measure delivery availability or enable the debug fault in production.
It also does not replace a later independent-builder and signed-release
provenance exercise.

