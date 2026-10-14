# RFC-0111: Host-neutral Cargo identity verification (M0.9.88)

Status: accepted externally.

## 1. Problem

The independent-builder verifier compared the complete `cargo -vV` strings
from the local and GitHub-hosted builders. Cargo includes an `os:` line in that
output. The line identifies the Windows release on which Cargo is running, not
the Cargo executable or the Rust toolchain selected by `rust-toolchain.toml`.

GitHub run `38506048297` for revision
`bb4b7a08184301c2f6b56eed658fdae483cd8f55` produced the exact expected
2,432,000-byte executable with SHA-256
`d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`
and created attestation `55680587`. The production verifier initially rejected
that otherwise valid evidence because the local Cargo record named Windows 11
while the hosted record named Windows Server 2022.

## 2. Canonical Cargo identity

The verifier now parses the closed eight-line `cargo -vV` shape. It requires
and compares byte-for-byte:

- Cargo version and short commit;
- release;
- full commit hash and commit date;
- target host triple;
- libgit2 build identity;
- libcurl and TLS backend identity.

The eighth line must still be a 64-bit Windows `os:` line, but its Windows
release text is deliberately excluded from cross-host equality. The independent
record separately remains constrained to a GitHub-hosted Windows runner. Rustc
`-vV`, the hash-locked MSVC/SDK libraries, observed native link inputs, bundled
LLD, path-remapping policy and the executable remain exact-equality checks.

This is not a general normalization hook. An unknown line count, malformed
field, non-Windows host, changed Cargo commit, library identity or target triple
fails closed.

## 3. Regression boundary

The verifier self-test uses two records whose only Cargo difference is the
Windows 11 versus Windows Server 2022 `os:` line and requires acceptance. It
then changes the Cargo commit hash and requires rejection. The static
independent-builder boundary pins both outcomes and the canonicalization
function.

Evidence format remains version 6 because no serialized field or meaning
changes: both records still retain the full observed `cargo -vV` output. Only
the verifier's comparison correctly distinguishes tool identity from runner OS
identity.

## 4. Acceptance and stage boundary

Before acceptance, the exact committed verifier revision must be reproduced in
two local clean roots and by a fresh GitHub-hosted Windows run. Production
verification must validate the executable, builder record, native lock and
native-link manifest attestations for that exact revision.

The implementation changes no messenger runtime, protocol, wire format, IPC or
executable target. It creates no ZIP, background service or Git tag. The
required evidence refresh performs only the bounded `kilogram-offline` release
builds already defined by the independent-builder contract.

## 5. External result

Revision `68804c93f43b405008cfecf35dc8660db360a3a3` produced two identical
local clean-root executables and GitHub run `38507114425` independently
produced the same 2,432,000-byte executable with SHA-256
`d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
The equality gate created attestation `55683374`.

The corrected production verifier accepted the complete downloaded format-v6
record and independently verified GitHub attestations for the executable,
builder record, native-toolchain lock and observed native-link manifest. This
closes M0.9.88; the earlier run remains diagnostic evidence only.
