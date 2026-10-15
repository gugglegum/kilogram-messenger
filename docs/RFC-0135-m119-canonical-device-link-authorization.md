# RFC-0135: Canonical device-link authorization (M1.1.9)

Status: accepted as an inactive deterministic-CBOR candidate.

## 1. Purpose

M1.1.9 closes the remaining account-authority schema gap with the Root-signed
authorization that enrolls a requested device. The candidate preserves the
active ceremony's essential binding: one Account Root authorizes one exact
request, one exact device certificate and one exact resulting device list.

The active bootstrap application continues to use versioned Postcard. No
current protocol identifier selects this candidate.

## 2. Canonical shape and binding

The content contains the Account ID, the 32-byte request ID, the exact canonical
device certificate and the exact canonical account device list. Nested objects
are byte strings bounded to 524288 bytes, so their already-signed encodings are
not reinterpreted or normalized by an outer codec.

The outer Ed25519 signature uses a new `cbor-v1` domain. Verification checks
both nested signatures, Account linkage, and that the device list contains the
exact certificate. Merely finding the same Device ID is insufficient.

Binding the request ID prevents replaying a valid enrollment response into a
different device-link ceremony. The encrypted response envelope and request
itself remain separate coverage work; this milestone covers the authorization
object registered by the account-authority compatibility surface.

## 3. Conformance and resource bounds

Rust and an independent JavaScript/Node Ed25519 implementation reproduce the
same content, signing preimage, signature and envelope bytes. The strict decoder
joins the bounded mutation corpus, increasing it from 1827 to 2088 malformed
cases. The aggregate ceiling rises from 2000 to 2500 while each fixture remains
limited to 128 truncations and 128 bit flips plus fixed structural cases.

The account-authority coverage surface is now complete. Aggregate activation
is still false: eighteen other registered surfaces are partial, not started or
explicitly not targeted, and the migration contract has additional gates.

## 4. Security boundary

This milestone changes no active wire bytes, device-link runtime behavior,
database format, release artifact or network policy. It assigns no protocol
identifier and does not permit format sniffing or Postcard fallback after a
canonical parse failure.
