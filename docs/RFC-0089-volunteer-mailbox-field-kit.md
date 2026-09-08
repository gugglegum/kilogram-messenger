# RFC-0089: Volunteer mailbox field kit (M0.9.67)

Status: implemented and network-free self-tested. External Alice/Bob evidence is
pending.

## 1. Outcome

A clean-HEAD Windows builder creates one ordinary directory containing a stable
debug `kilogram-cli.exe`, bounded public scripts, the Russian procedure, exact
artifact hash and precomputed static boundary evidence. It creates no archive,
release build, installer, scheduled task, service or additional executable and
does not launch a network endpoint.

The field procedure covers two store/transport-distinct providers, sender-side
two-of-three receipts, an explicit stopped-Alice IPC boundary, recipient Iroh
LIST/commit/DELETE from both stores, Bob restart and exact single-message local
history. A fail-closed verifier correlates all public evidence rather than
treating an isolated success line as completion.

## 2. Private provider bootstrap

Two provider-only test identities may be initialized in separate local private
directories on one operator host. Each gets its own Account Root, Device key,
state, launch profile, IPC descriptor, Iroh endpoint and blind store key. The
bootstrap never places these secrets in the kit or evidence directory. Only the
store-signed expiring offer and bounded runtime log are exported.

This arrangement proves protocol mechanics and transport/store diversity. It
does not prove distinct operators, physical failure domains, Sybil resistance
or honest retention. A stronger deployment test must place providers under
independent operators.

## 3. No-clobber evidence sequence

Every run uses a new evidence directory and unique message marker. Scripts
refuse to overwrite provider logs, offers, imports, queue output, sender-offline
boundary, restart history or final evidence. The ordered phases are:

1. start two providers and export fresh 15-minute offers;
2. import both offers through authenticated local IPC on Alice and Bob;
3. stop Bob, queue one marker on Alice and wait for two signed PUT receipts;
4. stop Alice and prove its IPC is unreachable;
5. start Bob and observe both volunteer Iroh commits and signed deletions;
6. restart Bob, require no volunteer redelivery and capture one history entry;
7. verify all logs against the manifest and clean-build boundaries.

The kit never copies runtime profiles, IPC bearer descriptors, Account Root,
state, seed or vault keys. Test message content is deliberately public evidence
and must not contain real conversation data.

## 4. Current compatibility boundary

M0.9.67 retains the existing HTTPS mailbox upload. The field run therefore
requires an already active Alice/Bob mailbox binding and its reachable HTTPS
compatibility store even though Bob's tested retrieval source is the volunteer
Iroh path. This avoids removing the proven delivery path before external
evidence exists.

A successful same-host-provider run is enough to continue implementation, but
not enough to claim server-free production availability. Exact scalable replica
location, provider-set commitment, independent-operator evidence and making
HTTPS optional remain later decisions.

## 5. Verification

`verify-kilogram-volunteer-field-evidence.ps1 -SelfTest` creates and verifies a
bounded synthetic evidence set. `verify-kilogram-volunteer-field-kit-boundary.ps1`
checks clean HEAD, stable debug artifact, no archive/release/network action,
private provider setup, no-clobber capture, stopped-sender proof, evidence
correlation and no-new-EXE boundary. The provider bootstrap itself is exercised
against a temporary private directory without starting runtime or network.

The executable operator procedure is
`docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md`.
