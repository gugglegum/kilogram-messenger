# RFC-0131: Independent digests and bounded decoder mutations (M1.1.5)

Status: accepted as an inactive conformance and regression baseline.

## 1. Purpose

M1.1.5 closes the previously explicit independent-BLAKE3 gap and adds a
deterministic malformed-input corpus for the strict canonical decoders. It does not activate canonical CBOR or replace the production Rust `blake3` crate.

## 2. Independent BLAKE3 oracle

`scripts/lib/blake3-reference.mjs` is an intentionally small, unoptimized
JavaScript port of the official BLAKE3 reference algorithm in hash mode. It has
no package dependency and is used only by conformance scripts.

Before checking Kilogram data, the oracle must reproduce the first 32 output
bytes from official BLAKE3 test vectors for lengths 0, 1, 1023, 1024, 1025 and
2048. Those cases exercise empty input, a partial block, both sides of the
1024-byte chunk boundary and a two-chunk tree. The bounded copied subset and
its upstream source/license are recorded in `BLAKE3-REFERENCE-VECTORS.json`.

Only after that self-check does the oracle independently reproduce five
Kilogram values: canonical event ID, prekey-pool ID, recovery ID and two
recovery checkpoint IDs. Ed25519 remains independently checked through Node's
native `crypto` implementation.

## 3. Bounded malformed-input corpus

`CANONICAL-DECODER-MUTATIONS.json` is the closed corpus manifest. Five golden
objects cover authority, session, ratchet pool/directory and recovery. For each
fixture the test uses deterministic grids of at most 128 truncation and 128
bit-flip positions, then exercises trailing data, non-minimal version encoding,
indefinite arrays, reserved CBOR additional information and oversized declared
byte strings.

The run is deterministic, offline and bounded to 3,000 mutations; after the
M1.1.10 authorized-event addition the current corpus contains 2,610. Every mutation must
fail closed and the valid source
fixture must still verify cryptographically. The limit keeps ordinary Windows
CI responsive and makes the test repeatable.

## 4. Security boundary

This is stronger than hand-picked parser examples but is not a substitute for
continuous coverage-guided fuzzing, sanitizers or independent audit. Sustained
fuzzing remains an activation requirement. No network, runtime wire identifier,
database format, release artifact or user-visible behavior changes.
