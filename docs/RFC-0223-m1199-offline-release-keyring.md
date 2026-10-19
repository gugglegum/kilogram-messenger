# RFC-0223: Offline release trust root and bounded key rotation

Status: M1.1.99, verified locally, dormant.

## Why one pinned operational key is insufficient

The M1.1.90 local rollout store pins one Ed25519 policy authority. That is a
safe prototype boundary, but treating the same online-capable key as a
permanent release root would make routine rotation and incident revocation
needlessly dangerous. M1.1.99 therefore separates a long-lived offline trust
root from bounded operational release-policy keys.

No production root is embedded. No private key is stored in the repository.
The new module is pure verification logic: it has no filesystem, runtime, IPC,
network, desktop or CLI consumer.

## Complete root-signed snapshots

The offline root signs a canonical Postcard keyring snapshot. Each snapshot
contains:

- a strictly monotonic epoch and exact previous keyring ID;
- at most eight operational Ed25519 public keys;
- a deterministic key ID derived from the exact public key;
- immutable introduction epoch and signing permissions;
- an optional irreversible revocation epoch.

The list is complete, strictly sorted and duplicate-free. A successor cannot
remove or mutate an old certificate, un-revoke a key, backdate an addition or
skip/fork the predecessor. Newly introduced keys must name the current epoch;
new revocations must also name the current epoch. At least one active key must
retain containment permission, so normal-key compromise does not remove the
ability to publish an emergency stop policy.

Whole-chain rollback still needs a durable store or an external monotonic
witness. This milestone defines and tests the authenticated chain; it does not
claim that pure bytes can detect replacement by an older internally valid
snapshot.

## Narrow permissions

An operational certificate independently grants normal-policy and
containment-policy signing. A containment-only key cannot authorize a normal
rollout policy. Unknown, revoked or wrong-purpose signers fail closed.

M1.1.99 does not yet change `SignedReleasePolicy`. M1.2.0 will bind a policy to
the exact verified keyring snapshot and signer certificate, including rotation
between successive policies. Until then the existing single-key rollout store
remains the only local prototype and the new keyring is dormant.

## Verification and non-claims

Six new unit tests cover root substitution, byte tamper, purpose separation,
rotation, irreversible revocation, unknown/duplicate keys, fork/epoch skip and
the eight-key bound. Together with the existing controller/store tests the
crate passes 31/31 tests, rustfmt and locked/offline strict Clippy.

This milestone does not add policy download, automatic enrollment, telemetry
upload, production keys, runtime activation or a public rollout. Peer wire,
sync wire and runtime IPC are unchanged; the public default remains
`legacy-only`.
