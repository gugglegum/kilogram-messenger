# RFC-0156: canonical runtime IPC contact enrollment and device-directory status

Status: implemented as an inactive M1.1.30 candidate.

## Summary

M1.1.30 extends the deterministic-CBOR runtime IPC candidate with two narrowly
scoped operations:

1. `add-contact` enrolls one exact peer Account from an authenticated endpoint
   descriptor and returns the resulting canonical contact identity;
2. `own-device-directory-status` reads the currently applied own-device roster,
   its authority revision, convergence state and authenticated restart receipt.

This RFC does not activate canonical CBOR. The shipping runtime remains Postcard
IPC version 26 with JSON descriptor version 7. It also deliberately excludes the
authority-changing `apply-own-device-directory` operation: a read-only status
projection and a roster mutation have different trust boundaries and must not be
reviewed or activated as one capability.

In other words, the active transport is still Postcard IPC version 26.

## Contact enrollment

The request commits to the local signed runtime descriptor, a fresh request ID,
the descriptor bearer token, a lifetime of at most 30 seconds, a bounded UTF-8
conversation label, the exact expected peer Account and a bounded UTF-8 endpoint
descriptor path. The peer Account must be nonzero and must differ from the local
Account named by the runtime descriptor.

The response is signed by the exact Device named by the descriptor and commits
to the digest of the complete request. A successful result repeats the peer
Account and Device, reports one to four resulting endpoint candidates, and
distinguishes whether a candidate and contact record were newly inserted.

The canonical contact ID is domain-separated and hashes the deterministic-CBOR
array containing the profile version, local Account, peer Account and the
label-derived Conversation ID. It intentionally differs from the identifier in
the active Postcard implementation. Activation therefore requires an explicit
mapping or migration; silently reinterpreting an existing contact ID is not
permitted.

The canonical layer treats the descriptor path as bounded portable UTF-8. A
platform adapter must additionally require an absolute, application-owned,
non-symlink path and must apply its normal filesystem access policy before the
command is executed.

## Own-device-directory status

The status response binds the descriptor Account and local Device and exposes:

- a bounded state-directory path;
- a positive authority revision;
- one to 32 active Devices, matching the Account identity and canonical
  device-list limits;
- an optional authenticated receipt ID and generation, which must appear as a
  pair and be nonzero;
- a nonzero digest of the applied device list;
- bounded applied and launch-profile device-list paths;
- whether the two profile paths have converged;
- whether restart recovery used the current receipt, a recovered receipt, or
  the launch-profile directory.

`Current` convergence requires byte-identical applied and launch paths;
`Required` requires them to differ. Receipt-based restart states require the
receipt pair, while launch-profile recovery forbids it. These relations are
validated before signing and after decoding.

## Authentication and bounds

Both commands reuse the signed loopback descriptor and short-lived bearer model
from RFC-0154. Responses use contact-specific request-digest, response-signature
and response-ID domains. The contact response is limited to 128 KiB, text and
paths to 4096 bytes, endpoint candidates to four, and the own-device roster to
256 Devices. Noncanonical CBOR, unknown variants, trailing bytes, expired
requests, descriptor substitution, response substitution and inconsistent
directory state fail closed.

Golden vectors cover contact enrollment and a divergent directory profile with
an authenticated current receipt. The independent JavaScript reference derives
the contact identity, request digests, signatures and response IDs without using
the Rust implementation. Both request and response forms are also part of the
bounded decoder mutation corpus.

## Activation boundary

M1.1.30 changes no active wire bytes, network ALPN, endpoint publication,
desktop behavior or accepted M1.0.0 tag. Canonical runtime IPC remains partial.
Activation still requires completing every command/result family, a versioned
dual-reader migration, downgrade tests, sustained fuzzing and fresh desktop/CLI
field evidence.
