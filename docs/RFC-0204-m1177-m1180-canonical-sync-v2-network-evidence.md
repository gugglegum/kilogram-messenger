# RFC-0204: M1.1.77–M1.1.80 canonical sync v2 network evidence

Status: implementation complete; exact hosted and physical result records are
collected only after the implementation commit is fixed.

## Purpose

The canonical `kilogram/m1/sync/cbor/2` state machine, typed stores, durable
client/responder adapters, mutual selection ceremony and rollback drill were
previously verified without an active network path. This stage adds one narrow
way to exercise those already closed components over real Iroh streams without
changing the production transport decision.

The active application still registers and dials only Postcard v6 at
`kilogram/m0/sync/8`. The canonical identifier remains absent from production
CLI/runtime ALPN lists, connection tickets and automatic downgrade logic.

## Evidence-only executable

`kilogram-cli` declares an empty default feature set. The additional binary
`kilogram-sync-v2-evidence` is built only with the explicit
`sync-v2-evidence` feature. A normal `kilogram-cli` build neither compiles nor
exposes this command surface.

The harness:

- creates fresh, test-only Account Root and Device identities;
- signs one distinct authorized legacy event for each role and imports it into
  the typed canonical carrier store;
- bootstraps the existing durable client and responder journals;
- verifies the exact mutually authenticated candidate selection before binding
  `/cbor/2`;
- registers exactly one ALPN per experiment process;
- drives the existing durable adapters over Iroh bidirectional streams;
- records bounded JSON reports without role secrets, event bodies or state
  directories;
- never attempts Postcard after a selected canonical decode, network or session
  failure.

For deterministic field runs, `--advertise-ip` may reduce a test ticket to one
IP address already present in the bound endpoint. It cannot invent an address
or port. This prevents VPN and virtual-adapter candidates from consuming the
entire direct-path deadline. Hosted runs instead bind explicitly to IPv4
loopback and remain a separate topology class.

Relay-only field evidence may additionally pass an explicit `--relay-url` to
the listener. The signed ticket then carries that exact home relay and the
dialer pins its own strict relay map to the ticket, avoiding an ambiguous
cross-relay retry. The override remains confined to the feature-gated harness.

The packaged executable is copied to the stable name `kilogram-cli.exe` in a
stable field-test path. This avoids producing a new Windows Firewall identity
for every evidence iteration; it does not make the evidence binary a production
CLI.

## Required experiment matrix

Each independent scenario starts from a fresh prepared directory:

1. canonical v2/v2 direct sync, ending with two events and identical inventory
   digests;
2. a deliberate stop after durable Diff receipt/transmission, followed by new
   processes resuming the same journals without duplicate events;
3. relay-only canonical sync through one explicitly pinned public relay when
   it is reachable, or
   a bounded explicit `unavailable` observation;
4. mixed-version agreement on Postcard before canonical selection;
5. corrupt canonical bytes after selection, proving unchanged history and zero
   legacy fallback attempts;
6. explicit one-step rollback, plus rejection of rollback while selected
   canonical evidence is active.

The restart checkpoint is intentionally asymmetric. Alice has already applied
the authenticated Diff and journals `SendEventBatch`, so her inventory contains
two events. Bob has durably sent that Diff but still journals
`ReceiveEventBatch`, so his inventory contains one event. A successful restart
must resume those exact actions and converge both roles to two events with the
same digest; requiring equal checkpoint inventories would test the wrong
boundary.

Hosted evidence uses two fresh processes on one Windows runner. Physical
evidence requires two distinct Windows host names and the same executable
SHA-256 on both hosts. The two evidence kinds are never interchangeable.

## Evidence manifest

`scripts/verify-kilogram-sync-v2-network-evidence.ps1` accepts one bounded JSON
manifest. It pins the schema, milestone, source revision, debug binary hash,
target, topology, production boundary and all scenario reports. It rejects:

- a mismatched source revision, binary hash, role, run ID or transport;
- reused state/run IDs across independent scenarios;
- physical evidence using one host or loopback;
- any legacy dial/fallback after canonical selection;
- missing convergence or the wrong durable restart checkpoint;
- unbounded or ambiguous relay failure claims;
- secret-bearing role fields, plaintext markers or state-file references;
- any claim that `/cbor/2` is registered or dialed by default.

Its self-test includes positive evidence and negative revision, fallback,
topology, secret-field, checkpoint-count, checkpoint-action and rollback-path
mutations.

## Hosted execution policy

`.github/workflows/canonical-sync-v2-evidence.yml` is manual-only. It builds a
debug harness with two Cargo jobs, runs the two-process matrix, verifies the
manifest and uploads only that small manifest. It does not run on push or pull
request and uploads no executable, state directory, identity, corpus or ZIP.

The general Windows readiness workflow is also manual-only. Hosted runs are an
explicit operator action rather than a recurring cost on every repository
update.

## Activation boundary

Passing this stage means `ready-for-explicit-activation-decision`, not
`activated`. The following remain false after evidence collection:

- `candidate_registered_by_default`;
- `candidate_dialed_by_default`;
- `legacy_fallback_after_selection`;
- `changes_active_wire`.

Any later dual-reader or prefer-canonical activation requires a separate
decision and change set. Postcard removal is outside this RFC.
