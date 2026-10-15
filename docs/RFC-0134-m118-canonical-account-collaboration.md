# RFC-0134: Canonical account collaboration artifacts (M1.1.8)

Status: accepted as an inactive deterministic-CBOR candidate.

## 1. Purpose

M1.1.8 extends the authority foundation with the two Root-signed artifacts
needed to bind messaging to an account and conversation: a complete device list
at one authority revision and a conversation membership snapshot.

The objects are specified by `CANONICAL-ACCOUNT.cddl` and
`CANONICAL-ACCOUNT-SCHEMA.json`. They do not replace the active Postcard
objects, and no current protocol identifier selects them.

## 2. Account device list

The list embeds the exact canonical authority snapshot and device certificates
as bounded byte strings. The outer Root signature therefore binds one exact
encoding of every nested object while each nested signature remains separately
verifiable.

The list is non-empty, bounded to 32 devices and strictly ordered by Device ID.
Every certificate must belong to the snapshot Account, predate its revision,
contain the complete messaging capability set and not appear in its revocation
set. The Account Root signs the complete list content with a new `cbor-v1`
domain.

## 3. Conversation membership

Membership is Root-signed by the owner Account. Its revision is positive, the
member list is bounded to 4096 Account IDs, strictly ordered without duplicates,
and includes the owner. The conversation ID, revision, owner and complete
membership are covered by the signature.

## 4. Conformance

The golden fixture reuses the canonical authority certificate and snapshot,
then adds a second deterministic public test Account. Rust and an independent
JavaScript implementation reproduce exact CBOR bytes and Ed25519 signatures.
Both strict decoders are included in the bounded mutation corpus, increasing
it from 1305 to 1827 malformed cases.

The `account-authority` coverage surface remains partial only because the
device-link authorization artifact is still missing. This milestone changes no
active wire bytes, runtime behavior, database format or release artifact.
