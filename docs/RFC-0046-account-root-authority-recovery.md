# RFC-0046: Portable Account Root authority recovery

Status: implemented in M0.9.19 (2026-09-04).

## 1. Goal

The 24-word phrase introduced in M0.9.16 reconstructs the Account Root signing
key, but it does not reconstruct the monotonic authority sequence, revocations,
the complete device list, or conversation membership heads. Recreating those
files at revision zero would let the same Root key sign a valid rollback or a
conflicting branch.

M0.9.19 therefore permits recovery only from three agreeing inputs:

1. the 24-word phrase;
2. a Root-signed portable authority package;
3. a separately retained latest Root-signed witness that pins the exact package.

Recovery publishes a new Root directory only after the whole candidate state
has been reconstructed and verified. It does not enroll a new device or restore
message history; the existing device-link and multi-source history ceremonies
remain separate steps.

## 2. Captured authority state

One package contains:

- the current Root-signed `AccountAuthoritySnapshot`, including the exact next
  authority revision and the complete canonical revocation set;
- the latest complete Root-signed `AccountDeviceListSnapshot`;
- every current Root-signed conversation membership head, sorted by
  Conversation ID;
- the Account ID and a format version;
- a Root signature over the complete package content under a dedicated domain.

The device list may legitimately have an older revision than the current
authority snapshot after a revocation. It may never be ahead of that snapshot.
Certificates issued but never published are deliberately absent, while their
consumed sequence numbers remain represented by the current authority revision.

Package size is bounded to 32 MiB and membership heads to 16,384. The opaque
binary format is `KILOGRAM-ARPKG01 || postcard(package-v1)`. It contains no
Root/device/vault key, recovery phrase, ratchet, message, or local history.
It does expose public-key and membership metadata and should still be treated
as privacy-sensitive.

## 3. Independent witness

The companion witness contains `(Account ID, authority revision, package ID)`
and a Root signature under a different domain. `package ID` is a domain-separated
BLAKE3 digest of the complete encoded package. Its opaque bounded format is
`KILOGRAM-ARWIT01 || postcard(witness-v1)`.

Restore requires an exact digest and revision match. Consequently an old
package cannot be combined with the latest witness, and pieces from different
accounts or exports cannot be mixed.

This witness is not a global monotonic service. If an attacker rolls back both
the package and the independently stored witness to an older matching pair,
offline recovery cannot detect that fact. The latest witness must therefore be
kept independently from the package and updated after every device enrollment,
revocation, or membership change. A hardware counter, transparency log, or
quorum of current devices is future work.

Possession of the phrase remains possession of the highest recovery authority.
The witness prevents accidental/stale branch reconstruction; it cannot stop a
phrase holder who can also replace the user's independently trusted checkpoint.

## 4. Export, inspect, and restore

The one-shot helper exposes:

```text
kilogram-bootstrap account-recovery-export \
  --account-root-dir <CURRENT_ROOT> \
  --package-file <NEW_PACKAGE.karp> \
  --witness-file <NEW_WITNESS.karw>

kilogram-bootstrap account-recovery-inspect \
  --package-file <PACKAGE.karp> \
  --witness-file <WITNESS.karw>

kilogram-bootstrap account-recovery-restore \
  --account-root-dir <NEW_ROOT> \
  --package-file <PACKAGE.karp> \
  --witness-file <WITNESS.karw> \
  --expected-package-id <INSPECTED_PACKAGE_ID> \
  --expected-authority-revision <INSPECTED_AUTHORITY_REVISION> \
  --recovery-phrase-stdin
```

Export holds the same exclusive Account Root authority lock as enrollment,
revocation, and membership mutation. Both outputs must be new regular paths
outside the Root directory. Temporary files are synced before no-clobber
publication; failure to publish the witness removes the package created by that
attempt.

Inspect verifies all nested and outer Root signatures plus the exact witness
binding without reading the phrase. Inputs are bounded regular files and direct
symlinks are rejected.

Restore reads the phrase only from standard input, derives the Account ID, and
requires it to match the authenticated package. It also requires the package ID
and authority revision shown by inspect; both are recalculated after reading the
artifacts so a valid same-path replacement between inspect and restore fails
before staging begins. The destination must not exist. The helper creates a
same-parent staging directory, writes a fresh platform Root-key envelope plus
the exact sequence, revocations, device list, and membership heads, regenerates
and compares all authority views byte-for-byte, then performs one no-clobber
directory rename. Any error leaves the destination absent. On Windows the
recovered Root key is wrapped anew with DPAPI CurrentUser.

## 5. Required recovery sequence

1. Obtain the newest package and independently retained newest witness.
2. Run offline inspect and compare its Account ID/revision/package ID with the
   user's recorded checkpoint where available.
3. Restore into a new Root directory using the phrase through standard input.
4. Keep the old Root untouched until the recovered state has been inspected.
5. Use the normal SAS-confirmed device-link ceremony to enroll a replacement
   device; then recover its history from one or more existing devices.
6. Immediately export and independently retain a new package/witness pair after
   the authority revision changes.

The implemented desktop orchestration is specified in RFC-0047.

Recovery does not overwrite an existing Root, silently reset authority state,
copy any device secret, claim message-history completeness, or revoke a lost
device automatically.

## 6. Verification

Automated regression covers a Root with two devices and an updated membership,
exact export/inspect/restore, equality of authority/list/membership state, and a
strictly newer enrollment after restore. It also rejects a wrong phrase,
tampering, output inside the Root, an existing destination, and an old package
paired with a newer witness. Workspace formatting, strict Clippy, serial tests,
release build, and a real helper process smoke are required for completion.
