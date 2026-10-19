# RFC-0224: Rotation-aware trusted release-policy bundle

Status: M1.2.0, verified locally, dormant.

## Self-contained verification

M1.2.0 binds one signed rollout policy to the exact root-signed keyring and
operational signer that authorize it. The bounded canonical Postcard bundle
contains the complete signed keyring bytes, their exact keyring ID, the exact
operational signer key ID, the claimed narrow purpose and the signed policy
bytes. The whole bundle is capped at 8192 bytes and must re-encode byte for
byte after verification.

Verification starts at an explicitly supplied offline trust-root public key.
It then verifies the complete keyring, recomputes its ID, authorizes the named
active operational key for the claimed purpose, verifies the existing release
policy signature and checks that policy disposition matches that purpose.
Replacing a keyring, signer, purpose or policy fails closed.

## Rotation without a permissive fallback

The pre-existing `SignedReleasePolicy` successor constructor assumed the same
signing key on both sides. It now also has an explicit variant that verifies
the predecessor with its previous authority before signing the next record
with a different authority. The old same-key API remains strict and unchanged.

A trusted bundle successor may keep the same verified keyring or move through
exactly one root-signed keyring successor. The policy itself must still advance
by exactly one policy epoch and name the exact previous policy ID. Skipped or
forked keyrings, revoked signers and stale/forked policies fail closed; there is
no legacy single-key fallback after bundle verification begins.

## Emergency key is deliberately weaker

A containment-only operational key may sign a containment policy that stops
new rollout. It cannot sign a normal policy that expands enrollment. Claimed
purpose is not trusted on its own: it is checked both against the certificate
permissions and the decoded policy disposition.

## Verification and non-claims

Five bundle tests cover canonical self-contained verification, foreign root,
keyring/purpose substitution, exact operational-key rotation, prohibited
expansion by the emergency key and authorized containment. The full rollout
crate passes 36/36 tests, rustfmt and locked/offline strict Clippy.

The bundle has no durable store, inbox, download path or runtime/desktop/CLI
consumer yet. No production root is embedded and no automatic enrollment is
enabled. Peer wire, sync wire, runtime IPC and public `legacy-only` behavior
remain unchanged.
