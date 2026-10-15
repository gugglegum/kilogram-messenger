# RFC-0148: M1.1.22 canonical publication-conflict resolution

Status: accepted locally, canonical candidate not active

## Outcome

M1.1.22 completes the canonical `publication-conflict` surface begun in
M1.1.21. The evidence objects are now followed by a Device-signed resolution
request, a Root-signed resolution, a self-contained response, and compact
portable request/response claims. This does not activate the candidate codec:
the running implementation continues to use its Postcard/base64url contract.

## Trust boundary

The requester signs the exact conflict proof, current Account authority
revision, peer identity, old and replacement publication write keys, strictly
increasing channel epoch, route policy, replacement ticket, and request time.
The request does not carry authority that it can authorize itself with. A
verifier must supply the current Root-signed device list and must confirm that
both the requester Device and the conflict detector are members of that exact
roster revision.

The Account Root signs a compact resolution containing the request ID,
conflict evidence ID, all peer and rotation fields, the route policy, and a
domain-separated digest of the replacement ticket. The self-contained
response adds the exact replacement ticket, so the recipient can check the
digest without trusting transport metadata.

## Portable claim

Both the request and response can be represented as strict canonical CBOR and
as an unpadded base64url URI under
`kilogram://publication-conflict/cbor-v1/`. The two claim kinds have distinct
artifact IDs but share a 12-byte confirmation value rendered as six uppercase
groups after the `KPC1` prefix. That lets two users compare one short value
across a separate channel without confusing a request with a completed Root
authorization.

A portable claim is an auxiliary comparison aid. It is not authority and does
not replace verification of the Device signature, current Root-signed device
list, Root resolution signature, or exact response binding.

## Fail-closed properties

- deterministic CBOR decoders reject trailing data, indefinite forms,
  non-minimal integers, unknown variants, invalid UTF-8 tickets, and bounded
  object overflows;
- the old and new write keys must differ and derive distinct publication
  channel IDs;
- the replacement epoch must strictly exceed the old epoch;
- the resolution must reproduce the request and evidence binding exactly;
- the response ticket must reproduce the Root-signed replacement digest;
- request and response claims must match the exact signed artifact kind;
- authority substitution, Root substitution, response-ticket mutation, and
  cross-kind claim substitution fail closed.

The independent Node reference derives every CBOR byte, Ed25519 signature,
BLAKE3 identifier, URI, and `KPC1` code without calling Rust. The bounded
decoder corpus includes all seven publication-conflict object classes and now
contains 11627 deterministic mutations under the unchanged 12000 ceiling.

## Coverage and activation

The coverage registry now records complete `publication-conflict` coverage:
13 of 20 source surfaces are complete, none are partial, five remain not
started, and two are deliberately not targeted by this migration. Complete
coverage of one surface is not global activation readiness. Protocol
identifier allocation, migration interoperability, and the remaining source
surfaces remain separate gates.
