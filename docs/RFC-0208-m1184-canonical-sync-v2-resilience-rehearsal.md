# RFC-0208: M1.1.84 canonical sync v2 resilience rehearsal

Status: local half accepted; exact-revision two-host rehearsal pending.

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

## Remaining acceptance work

The local run used an uncommitted implementation binary and therefore is a
development rehearsal, not the final exact-revision field artifact. The next
step is to commit this implementation, rebuild one stable debug executable and
run the relevant matrix across the primary PC and the OpenSSH-connected Windows
laptop. Both hosts must report the same source revision and executable SHA-256.

Only after that field manifest and the aggregate M1.1.85 regression pass may the
milestone status become `canonical-sync-v2-opt-in-dark-launch-verified`. The
public default remains `legacy-only`; desktop rollout, canonical-native history
and legacy retirement remain separate decisions.
