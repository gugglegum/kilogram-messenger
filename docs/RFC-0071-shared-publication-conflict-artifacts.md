# RFC-0071: Shared publication-conflict artifacts (M0.9.49)

Status: implemented in M0.9.49.

## 1. Problem

M0.9.48 separated the offline Root signer from the networked application, but
the online CLI and `kilogram-offline` still independently described the same
signed publication-conflict wire objects. Cross-component tests detected many
forms of drift only after a change had been made. A field-order, domain,
version, bound or verification-rule mismatch could make a valid incident
artifact unreadable on the isolated Root host or, worse, make the two sides
assign different meaning to the same bytes.

M0.9.49 gives that complete signed chain one implementation without adding a
network, runtime or image surface to the offline trusted computing base.

## 2. Shared boundary

The new `kilogram-publication-conflict` crate owns:

- signed ticket publications and signed observations;
- canonical publication and observation IDs;
- detector-signed conflict proofs and stable evidence IDs;
- Device-signed resolution requests;
- Root-signed resolutions and self-contained responses;
- KPC1 confirmation commitments and compact QR claim payloads; and
- all corresponding versions, domains, size bounds, encoders and verifiers.

Publication and observation are part of this boundary because a conflict proof
contains the exact observations and their serialized bytes contribute to proof
and evidence IDs. Extracting only request/response would therefore still leave
two implementations below the signed trust boundary.

The crate depends on `kilogram-identity` and
`kilogram-ticket-publication`, plus serialization/hash primitives. It does not
depend on Iroh, Tokio, Reqwest, image/QR libraries, runtime IPC, state, store,
session, ratchet or application protocol crates.

## 3. Adapters that remain outside

The online application still owns:

- Iroh endpoint parsing and connection tickets;
- HPKE encryption of publications;
- HTTPS opaque-store access;
- transport route selection and live runtime mutation; and
- QR image rendering/decoding and filesystem ceremony policy.

`kilogram-offline` still owns bounded regular-file/no-symlink handling, Root
loading, operator reports and QR image handling. Its Iroh-free v11 connection
ticket compatibility verifier intentionally retains raw endpoint JSON so it can
reconstruct the exact Device-signature bytes without linking Iroh. It verifies
that transport object and then passes only its authenticated public fields to
the shared conflict artifacts.

## 4. Wire compatibility

No wire version or signing domain changes in this milestone. The extracted
structs preserve the previous field order and serde representation for:

- ticket publication v1 and observation v1;
- conflict proof v1 and resolution request v1;
- Root resolution v2 and response v1; and
- publication-conflict QR claim v1 and KPC1.

`PublicationConflictRoutePolicy` is a small transport-independent enum with the
same variant order and kebab-case representation as the former online/offline
copies. The online transport maps its own `RoutePolicy` explicitly at the
boundary; the artifact crate never decides or opens a route.

## 5. Enforced dependency and ownership gates

`scripts/verify-publication-conflict-boundary.ps1` checks the locked normal
dependency graph and rejects network, runtime, image and unrelated Kilogram
packages. It also scans every other Rust source for duplicate definitions of
the signed wire objects or the KPC1 domain. At implementation time the shared
graph contains 94 dependency records and all denied categories are absent.

`scripts/verify-kilogram-offline-boundary.ps1` additionally requires the shared
crate and still rejects the offline binary's network/runtime dependencies. The
offline graph now contains 206 dependency records; this small metadata increase
does not add a network or runtime package.

## 6. Verification

The shared crate has a focused request/response claim regression. A second
network-free regression creates publications, conflicting observations, proof,
request and Root response, then requires online and offline inspectors to
produce identical IDs, artifact digests and KPC1 values. Response decode and
re-encode must remain byte-exact.

Workspace check, strict Clippy, release build and the dependency/duplicate gates
remain release checks. Network-bearing Cargo harnesses are compiled but are not
started as part of this milestone: on Windows they must be run later through
the stable-name harness at an agreed time to avoid an unexpected Firewall
dialog.

## 7. Residual risk and next step

One code owner removes format drift between the two applications, but it does
not prove that a downloaded executable corresponds to the reviewed source.
The deterministic ZIP process still reuses one locally built binary. The next
hardening slice is a pinned, independently repeatable offline build with
machine-verifiable provenance and comparison of artifacts from separate build
roots. Hardware-backed/non-Windows Root providers remain later platform work.
