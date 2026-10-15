# RFC-0152: Canonical Account Root recovery package and freshness quorum

Status: implemented as an inactive M1.1.26 candidate.

## 1. Scope

This stage gives the existing Account Root recovery package, its independent
witness, a fresh approval request and current-device approvals deterministic
CBOR representations. It does not activate a new reader or writer and does not
change the active Postcard v1 artifacts.

Recovery-policy epoch transitions are deliberately excluded from this stage.
Ordinary freshness approval asks the current electorate whether one exact
package is current. Changing that electorate requires joint consent from the old and new rosters
and is a separate canonical surface.

## 2. Exact authority package

The Root-signed package contains the Account ID, exact current Root-signed
authority snapshot, an exact Root-signed device list built over the same
authority snapshot, and every Root-signed membership head owned by this
Account. Memberships are strictly ordered by Conversation ID.

The package signature covers all nested canonical bytes. Its ID is BLAKE3 over
the complete signed package with an independent domain. The state-vector digest
covers the exact snapshot, list and membership sequence. The recovery-roster
digest covers the complete signed device-list bytes.

## 3. Independent witness

The witness is separately Root-signed and pins `(Account ID, authority
revision, package ID)`. Verification requires an exact match to the package.
It catches accidental mixing or partial replacement, but a matching stale pair
can still be replayed if both storage locations are rolled back together.

## 4. Fresh current-device quorum

A request binds the exact package and witness, a non-zero 256-bit challenge,
issue time and expiry. Its lifetime is at most 1,800 seconds. The Request ID is
the domain-separated BLAKE3 digest of the complete request.

Every approval is signed by an active Device in the exact package roster and
binds the Request ID, Package ID, authority revision, state-vector digest,
roster digest, challenge, request expiry, the signer's preceding approval head
and approval time. Contextual verification rejects substitutions even if the
substituted object is independently well signed.

Approvals are canonical only when strictly ordered by Device ID. The report
distinguishes artifact-only verification, one current device, and a strict majority
(`floor(N/2)+1`). A quorum is not a Root signature and cannot authorize
an electorate change.

## 5. Bounds and activation

The package is limited to 32 MiB and 16,384 memberships; nested authority
objects are limited to 512 KiB. Witness, approval and approver counts retain
small explicit limits. Decoders accept only the deterministic-CBOR v1 profile,
verify every nested signature and reject trailing/non-minimal input.

Activation still requires canonical recovery-policy transitions, completion of
the targeted compatibility surface, an explicit protocol-version migration,
dual-reader and downgrade tests, sustained fuzzing and fresh field evidence.
