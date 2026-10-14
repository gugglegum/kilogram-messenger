# RFC-0127: Canonical wire schemas and golden vectors (M1.1.1)

Status: accepted as an inactive cross-language conformance baseline.

## 1. Purpose

RFC-0126 made every active Postcard version, ALPN, signature domain and ID
domain visible, but intentionally did not call the Rust/Serde layout a stable
cross-language protocol. M1.1.1 defines the first normative candidate encoding,
two positional object families and deterministic byte/signature/ID vectors.

This RFC does **not** switch the running messenger to the new encoding. The
active event and sync paths remain exactly as recorded in
`PROTOCOL-COMPATIBILITY.json`. A future activation must use an explicit new
version or parallel identifier and a reviewed migration; silently reinterpreting
an existing Postcard version is forbidden.

## 2. Encoding profile

The candidate profile is `kilogram-deterministic-cbor-v1`, a deliberately small
subset of the core deterministic encoding requirements in RFC 8949:

- preferred serialization is mandatory, so integer and length arguments use
  the shortest representation;
- every byte string and array has a definite length;
- only major type 0 (unsigned integer), major type 2 (byte string), and major
  type 4 (array) are accepted;
- maps, text strings, negative integers, floating point, tags, simple values,
  and indefinite-length items are rejected;
- schemas use fixed positional arrays, fixed-width identifiers and strict
  unsigned bytewise ordering for set-like identifier lists.

The restricted data model avoids map-key ordering, Unicode normalization,
floating-point equivalence and duplicate-key disagreement. It is still CBOR,
not a private cryptographic primitive. The normative external standard is
<https://www.rfc-editor.org/rfc/rfc8949.html>.

`CANONICAL-WIRE-SCHEMA.json` is the machine-readable positional schema. The
Rust validator additionally bounds nesting, total item count and byte-string
size before a future application decoder interprets the object.

## 3. Initial objects

The baseline contains five schema objects:

1. acknowledgement payload v1;
2. acknowledgement event content v1;
3. signed acknowledgement event v1;
4. sync inventory content v1; and
5. signed sync inventory v1.

Acknowledgement parents and sync inventory event IDs must be strictly increasing
under unsigned bytewise lexicographic comparison. Duplicate or differently
ordered representations of the same logical set therefore fail before signing.
The schemas reuse Ed25519 device signatures and BLAKE3 event IDs; no new
cryptographic algorithm is introduced.

The reserved, inactive domains are:

- `kilogram:event-signature:cbor-v1\0`;
- `kilogram:event-id:cbor-v1\0`; and
- `kilogram:sync-inventory-signature:cbor-v1\0`.

They intentionally differ from every active Postcard domain. A decoder cannot
mistake candidate bytes for an old signed transcript without an explicit bug in
the future migration layer.

## 4. Golden vectors

`CANONICAL-WIRE-VECTORS.json` contains one acknowledgement-event vector and one
sync-inventory vector. The seed `000102...1f` is public test material, never a
production secret. Each vector fixes:

- logical input identifiers and ordering;
- exact deterministic CBOR bytes;
- exact signature-domain bytes and full signing preimage;
- the deterministic Ed25519 public key and signature; and
- for the event, the signed-envelope bytes, ID-domain bytes and BLAKE3 ID.

Rust tests derive the public key from the test seed, rebuild both transcripts,
verify both Ed25519 signatures, recompute the event ID, and validate the
restricted CBOR profile. The PowerShell boundary independently implements the
small CBOR encoder and rebuilds both contents, preimages and signed envelopes
from the JSON inputs. It does not pretend that PowerShell independently
implements Ed25519 or BLAKE3; those values remain checked by the Rust tests and
are explicit inputs to later non-Rust implementations.

## 5. Fail-closed boundary

`scripts/verify-kilogram-canonical-wire-boundary.ps1` requires:

- exact inactive status and current runtime encoding `postcard`;
- the restricted RFC 8949 profile without weakened allowed types;
- all five schema objects in stable order;
- exact Rust profile/domain bindings;
- lowercase fixed-width vector inputs; and
- independently reconstructed canonical bytes and signing preimages.

Its self-test rejects an activation claim, a removed schema object, changed
golden bytes and non-canonical hexadecimal. The script is the 35th member of
the closed M1 readiness boundary inventory.

## 6. Activation and remaining work

Before any production path may emit these bytes, all of the following are
required:

1. complete schema coverage for the chosen protocol family, including nested
   certificates, authority snapshots and encrypted ratchet payloads;
2. a new explicit protocol version or parallel ALPN/artifact identifier;
3. bounded dual-reader migration and downgrade/cross-version negative tests;
4. at least one independent non-Rust implementation passing the same vectors;
5. fuzzing and malformed-input coverage for the full decoder; and
6. fresh field and hosted evidence for the activated surface.

Consequently M1.1.1 is an executable compatibility design checkpoint, not an
active wire change, a general CBOR decoder, an audit result or a production
security claim.
