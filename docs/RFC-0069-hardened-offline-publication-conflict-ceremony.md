# RFC-0069: Hardened offline publication-conflict ceremony (M0.9.47)

Status: implemented in M0.9.47.

## 1. Problem

M0.9.45 separated the online Device request from the offline Account Root
signature, and M0.9.46 removed runtime restarts. The remaining offline step was
still too easy to perform as a blind file-in/file-out command: an operator could
copy a `.pcrq` to the Root host and sign it without first seeing which account,
peer, evidence, channel transition and replacement ticket it authorized.

The full request and response can contain complete signed evidence, an authority
directory and a connection ticket. They may be several MiB and therefore must
not be represented as one QR code. This milestone adds a bounded public viewer,
a compact cross-channel claim and a mandatory human confirmation gate while
keeping Root material out of the desktop and online runtime.

## 2. Public inspection

Two commands require no state directory or Root material:

- `publication-conflict-request-inspect` authenticates the Device-signed
  request, complete conflict evidence and embedded fresh peer ticket;
- `publication-conflict-response-inspect` authenticates the Root-signed
  response and its exact embedded peer ticket.

The reports expose artifact size and BLAKE3 digest, request/resolution IDs,
local Account and authority revision, evidence and detector, peer Account and
Device, old/new publication channels and epochs, route, replacement-ticket
digest/Endpoint, peer authority revision/device count and relevant timestamps.

Inputs use the existing bounded regular-file readers. QR input is additionally
limited to one non-symlink PNG/JPEG file, 16 MiB, 4096 by 4096 pixels, a bounded
decoder allocation and exactly one detected QR symbol.

## 3. Compact QR claim

An inspector can emit a no-clobber PNG whose ASCII payload begins with
`kilogram://publication-conflict/v1/`. The payload contains a versioned postcard
claim with:

- artifact kind and artifact ID;
- request ID, local Account ID and authority revision;
- stable conflict evidence ID;
- peer Account/Device IDs;
- old/new channel IDs;
- exact replacement-ticket digest; and
- the KPC1 confirmation code.

The QR is deliberately a compact verification claim, not the `.pcrq` or
`.pcrp`. Full signed artifacts still travel on removable media. Supplying
`--verification-qr-file` makes the inspector or signer decode the QR and require
field-for-field equality with the independently decoded signed artifact.
Request and response claims use distinct kinds and artifact IDs, so they cannot
be substituted for each other.

## 4. Human confirmation code

KPC1 is a 96-bit truncated BLAKE3 commitment, domain-separated and
length-prefixed over the common request/response security fields: request ID,
local Account/revision, evidence ID, peer Account/Device, old/new channels and
replacement-ticket digest. It is displayed as six four-hex groups:

`KPC1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`

The request and its valid response produce the same code. The code is intended
for exact comparison through a separate visual, voice or otherwise independently
authenticated channel. It is not a password, MAC or replacement for artifact
signatures.

`account-publication-conflict-authorize` now requires `--confirm-code`. It first
performs public request inspection and optional exact QR matching, then compares
the code case-insensitively. A mismatch fails before the Account Root directory
is resolved or loaded. Only after this gate does the command load Root state,
check the exact current requester/detector roster, sign the resolution and write
a new no-clobber response.

The untrusted request and new response must remain outside the Account Root
directory. The signer reports `runtime_state_loaded=false`; online request/apply
paths continue to report `root_secret_loaded=false`.

## 5. Desktop and IPC

Runtime IPC v20 adds the request artifact digest and KPC1 code to
`RuntimeIpcPublicationConflictRequest`. The Windows incident panel validates
their shape and displays:

- the prominent independent confirmation code;
- the full request digest;
- the public request-inspection/QR command;
- the exact offline authorization command including `--confirm-code`; and
- an optional public response-inspection/QR command before import.

The GUI still has no Root directory, phrase, key or signing operation. The
runtime remains online throughout the ceremony.

## 6. Compatibility

- Runtime IPC advances from v19 to v20.
- `.pcrq`, `.pcrp`, Root resolution v2 and ticket v11 remain unchanged.
- Endpoint-announcement bundle v4, acknowledgement v2, event, ratchet, sync and
  transport formats remain unchanged.
- The QR claim is a new auxiliary v1 format and is never accepted as a complete
  resolution artifact.

## 7. Verification

The three-Device regression now checks request digest/code reporting, request QR
round-trip and exact match, failure of a wrong code before a deliberately absent
Root can be loaded, rejection by the wrong Root, correct authorization with the
matching request QR, equal request/response KPC1 codes, distinct response QR,
cross-kind QR rejection, tamper rejection, live idempotent apply and unchanged
sibling convergence.

The QR unit test independently checks URI round-trip, equal common confirmation
codes and distinct request/response claims. Workspace formatting, strict clippy,
all tests and release artifacts remain release gates.

## 8. Honest boundary

This ceremony makes the operator's decision explicit; it does not make an
ordinary computer or removable medium trustworthy. If the artifact and QR/code
arrive through the same compromised channel, their agreement adds no
independent evidence. Malware controlling the display, keyboard or offline host
can mislead the operator or steal Root material.

KPC1 has 96 bits and is suitable for accidental/substitution detection with
exact human comparison, not for authenticating an unknown peer. The QR claim is
not separately signed; its authority comes only from an exact match to the
signed artifact and an independent trusted presentation.

The signer still lives in the general `kilogram-cli` binary and therefore has a
larger dependency and command surface than a purpose-built offline appliance.
A separately built minimal signer/viewer, reproducible media image, stronger OS
sandbox and hardware-backed Root remain future hardening layers.
