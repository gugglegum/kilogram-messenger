# RFC-0161: Canonical runtime IPC mailbox-capability status

Status: implemented as an inactive M1.1.35 candidate.

## Summary

M1.1.35 adds a deterministic-CBOR, read-only projection of the local mailbox
capability chains. A caller can request a bounded page of current signed
capability heads and their optional signed recipient acknowledgements. The
response is signed by the device named by the loopback runtime descriptor and
binds the exact request digest.

This candidate does not activate canonical CBOR. The shipped runtime continues
to use Postcard IPC version 26 and JSON descriptor version 7.

## Security boundary

The projection deliberately exposes only protocol objects that peers already
sign. Every returned head contains the exact signed capability update and, when
present, the exact signed acknowledgement. It never serializes a local mailbox
binding, a mailbox read secret, a mailbox write secret, or a decrypted peer
offer. An active grant may still contain the peer offer ciphertext sealed to
the recipient device; that ciphertext is retained byte-for-byte inside its
signed update.

The descriptor device must be one endpoint of every chain: either its account
and device are the owner, or they are the recipient. A third-party chain is
rejected. Updates, acknowledgements, signatures, chain identity, ordering,
action semantics, and acknowledgement-to-update binding are verified again at
the IPC boundary.

## Snapshot and pagination

Heads are ordered by the tuple `(owner account, owner device, recipient
account, recipient device, scope)` and duplicates are rejected. The snapshot
identifier commits to the local account and device, global aggregate counts,
and the complete ordered set of signed heads. A continuation cursor carries
that snapshot identifier and the next absolute index.

The adapter must build one immutable snapshot before serving a page. It must
not combine pages from different observations. If the requested cursor no
longer names the retained snapshot, the adapter returns a bounded error and the
caller restarts from the first page. A complete one-page response recomputes
both its aggregate counts and snapshot identifier; partial pages remain bound
to the device-signed global snapshot.

The fixed resource limits are:

- request: 64 KiB;
- response: 640 KiB;
- complete snapshot: at most 1,024 capability heads;
- page: at most 16 heads.

## Activation and compatibility

This RFC defines protocol bytes and verification semantics only. It does not
change the active runtime, desktop client, CLI, registry, or network behavior.
Activation still requires completion of the remaining runtime IPC surface, an
explicit IPC version migration, dual-reader and downgrade tests, sustained
fuzzing, and fresh field evidence.

No network was executed to verify this candidate. Golden vectors, an
independent Node.js reference, fail-closed boundary tests, and the bounded
decoder mutation corpus cover the new objects.
