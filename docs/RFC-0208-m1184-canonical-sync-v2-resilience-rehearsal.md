# RFC-0208: M1.1.84 canonical sync v2 resilience rehearsal

Status: accepted; exact-revision two-host rehearsal verified.

## Purpose

M1.1.83 connected the verified canonical sync v2 adapter to the ordinary
runtime under explicit Device-signed opt-in. M1.1.84 exercises the boundaries
that cannot be established by a happy-path convergence run: durable restart,
selected-path loss, explicit rollback before evidence and emergency containment
after evidence.

The rehearsal uses only fresh test Accounts, Devices and state directories. It
does not reuse historical state, relax authorization, change the public default
or publish a release artifact.

## Debug-only fault boundary

The ordinary debug binary recognizes
`KILOGRAM_TEST_STOP_CANONICAL_SYNC_V2_AFTER_DIFF_ONCE=1`. The switch is not an
activation authority: it cannot grant canonical permissions or create a
selection. It is accepted only by debug builds, is rejected by release builds
and is consumed once by the selected canonical client or responder.

The client-side form stops after the signed Diff and durable transition have
been accepted. The responder-side form stops after the signed Diff has been
sent. Both paths report `sync_transport_fallback_after_selection=false`; neither
starts a Postcard attempt. The next process must restore the persisted
checkpoint/frame and continue the exact selected session.

## Fresh local matrix

One debug binary completed seven fresh-state scenarios on 2026-10-18:

1. default/default selected `kilogram/m0/sync/8` and converged;
2. opt-in/default selected `kilogram/m0/sync/8` and converged;
3. opt-in/opt-in selected `kilogram/m1/sync/cbor/2`, persisted the same
   selection evidence on both roles and converged;
4. a client stopped after durable Diff, both processes restarted, the exact
   selection ID resumed and histories converged without duplicate or loss;
5. a responder stopped the selected connection after Diff, the client failed,
   no synchronized status or legacy retry appeared;
6. a Device explicitly rolled back exactly one state before any canonical
   evidence, producing epoch 4, `dual-reader-prefer-legacy` and no evidence;
7. both Devices bound the persisted selection evidence and entered emergency
   containment. The existing canonical session resumed with the same evidence,
   while a fresh conversation could start only on Postcard v6.

The successful bounded manifest is retained outside Git under
`.tmp/m1184-resilience-20261018T152502Z/M1184-LOCAL-EVIDENCE.json`. It contains
only hashes, counts, policy/transport outcomes and selection evidence IDs; it
does not contain state directories, keys or message bodies. The debug binary
SHA-256 was
`18fe1d407bb1a7c4d58fc739f1bf25c2e6c8a07604a71646f81a98baef690f6c`.

## Anti-rollback observation

The first containment rehearsal correctly failed when its second conversation
tried to reuse a previously observed prekey pool generation. This was not
suppressed. The harness was corrected to issue a fresh next-generation Bob
prekey pool, and the entire seven-scenario matrix was rerun from fresh state.
The accepted run therefore preserves the ratchet anti-rollback boundary rather
than weakening it for test convenience.

## Field diagnosis and bounded response deadline

The first exact-revision field attempt exposed a real orchestration defect. The
client's 20-second stream-I/O deadline also covered responder-side state-lock,
vault, store, import and durable-session preparation. A healthy responder could
therefore start writing the Diff only after the requester had already closed
the stream.

A feature-gated field probe sent 1, 32, 256, 1024 and 8123-byte responses over
the same production N0 direct path between the two physical hosts. Every size
arrived intact. This ruled out response-size, QUIC and Windows firewall causes.
The runtime now keeps the 20-second deadline for individual stream I/O but gives
bounded responder preparation and first response a distinct two-minute
deadline. The timeout remains fail closed and does not permit legacy fallback
after canonical selection.

## Accepted exact-revision field evidence

The complete two-host matrix passed from fresh state on source revision
`7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c`. One debug executable was used on
both hosts:

- SHA-256: `be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532`;
- size: 59,981,824 bytes;
- target: `x86_64-pc-windows-msvc`;
- Alice: `PAUL_HP_OMEN`; Bob: `DESKTOP-PAUL-RY`;
- transport: direct Iroh LAN; distinct physical hosts.

The accepted manifest is retained outside Git at
`.tmp/m1184-exact-20261018T165652Z/M1184-TWO-HOST-EVIDENCE.json`. It reports
`canonical-sync-v2-two-host-activation-rehearsal-verified`. Default/default and
mixed opt-in controls selected Postcard v6, mutual opt-in selected `/cbor/2`,
restart resumed the exact durable selection, containment preserved that resume
while routing a fresh conversation over Postcard, and selected-path loss exited
non-zero without synchronization or legacy retry. All successful histories
were equal.

M1.1.84 is complete. The aggregate M1.1.85 regression and ledger update remain
the final gate for status `canonical-sync-v2-opt-in-dark-launch-verified`. The
public default remains `legacy-only`; desktop rollout, canonical-native history
and legacy retirement remain separate decisions.
