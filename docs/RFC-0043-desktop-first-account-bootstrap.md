# RFC-0043: Desktop first-account bootstrap

Status: implemented in M0.9.16 (2026-09-04).

## 1. Goal and scope

M0.9.16 lets a first-run desktop client create a new Account Root and its first
enrolled device without copying private keys into the normal runtime profile or
runtime IPC. It establishes the process and storage boundary needed for later
device linking and recovery; it does not yet claim that a seed phrase alone can
safely reconstruct current account authority state.

This slice creates a new account only. Enrolling another device into an
existing account is deferred because issuing a certificate changes the durable
authority sequence and complete device list. That operation needs a resumable
transaction and authenticated transfer of the current authority history.

## 2. Recovery phrase and Account Root

A recoverable root starts with 256 random entropy bits encoded as a 24-word
English BIP39 mnemonic. The BIP39 seed, with an empty optional passphrase, is
passed through the domain-separated BLAKE3 derivation context
`Kilogram Account Root signing key v1`; the 32-byte result is the Ed25519 root
signing secret. Re-parsing the same canonical phrase therefore derives the same
public `AccountId`. Entropy, seed and intermediate secret buffers are zeroized.

The phrase currently encodes only the Account Root key. It does not contain:

- the monotonic authority sequence and complete revocation log;
- the latest root-signed device list or conversation memberships;
- any device, vault, ratchet or message-history key.

Consequently M0.9.16 validates a phrase and its `AccountId`, but deliberately
does not expose a "restore fresh root directory from phrase" operation. Resetting
authority state to revision zero could fork or roll back an existing account.
Future recovery must combine the phrase with an authenticated authority backup,
another current device or an independent monotonic witness.

## 3. Protected Account Root storage

`account-root-secret.key` is now a versioned bounded envelope instead of a raw
32-byte file. On Windows its payload is protected by DPAPI CurrentUser. On
platforms without an implemented native provider, the envelope explicitly
reports `plaintext-development`; this is not a production guarantee.

Loading an old raw 32-byte root key atomically rewraps it in the current platform
envelope without changing the `AccountId`. Invalid magic, oversized payload,
unsupported version/provider and DPAPI failure are typed fail-closed errors.
CLI account create/show now report the actual protection and load/migration
outcome rather than claiming unconditional plaintext storage.

## 4. One-shot bootstrap process

`kilogram-bootstrap` is a separate executable. Its `create --workspace-dir`
command accepts only a new destination path; no phrase or private key appears in
the command line. It builds this layout in a same-parent temporary directory:

```text
WORKSPACE/
    account-root/                 # protected root + authority history
    device/                       # encrypted DB-primary device state
    public/
        device-certificate.cert
        account-device-list.snapshot
        prekey-pool.bin
    bootstrap-receipt.json        # public metadata; never the phrase
```

The helper creates the root and device, issues and installs the first messaging
certificate and authority snapshot, publishes the complete one-device list,
creates a signed 16-entry seven-day prekey pool, migrates device state into the
encrypted vault and verifies that vault. Only then does one directory rename
publish the final workspace. An existing destination is never overwritten.

The persistent receipt contains IDs, absolute public paths, protection modes and
an explicit `root-key-only-authority-history-required` recovery scope. The
24-word phrase exists only in the bounded JSON process response. Its shared
contract has a redacted `Debug` implementation and zeroizes the phrase on drop.

## 5. Desktop first-run UX

The GUI discovers a sibling `kilogram-bootstrap(.exe)` or accepts an explicit
`--bootstrap-exe`. The first-run panel selects a non-existing account workspace
and runs the helper on the existing worker thread, keeping the window event loop
responsive. It validates the versioned response, size, absolute paths and exact
requested workspace before displaying the phrase.

After success the GUI:

- shows the phrase once and requires an explicit "saved offline" acknowledgement
  before removing it from process memory;
- displays the Account ID, first Device ID, root path, public prekey path and
  actual root/vault protection modes;
- fills the state, signed device-list, ticket and IPC paths in the secret-free
  launch-profile editor;
- leaves peer Account ID and peer prekey inputs empty until a real contact
  ceremony supplies them.

The GUI does not persist the phrase, automatically copy it to the clipboard or
write a launch profile before peer authorization is complete. Empty peer prekey
input is valid for the new local device; the runtime still validates all peer
material when contact setup occurs.

## 6. Security boundary

- The normal runtime process and IPC remain unchanged and never receive root,
  seed, device or vault secrets.
- The bootstrap executable owns secret-bearing account creation and exits after
  one response.
- The receipt, public exports and launch-profile draft contain no recovery
  phrase or private key.
- Replacing the configured helper executable is equivalent to replacing the
  application binary and is outside the local process boundary.
- DPAPI protects local-at-rest root material from offline copying, not from
  malware executing as the same Windows user.

## 7. Verification

Tests cover deterministic phrase-to-`AccountId` derivation, checksum/word-count
rejection, envelope persistence, legacy raw-key migration, redacted and bounded
bootstrap response parsing, atomic no-clobber workspace creation, public receipt
without phrase, valid public artifacts and a verified encrypted device vault.
Desktop tests cover explicit helper selection and a launch profile with no peer
prekeys. Workspace formatting, strict Clippy, all tests and a release build are
required before the milestone commit.

## 8. Next stage

M0.9.17 should implement existing-account device linking as a two-device
ceremony: short-lived recipient-bound authorization, complete current authority
state, transactional certificate/device-list publication, encrypted transfer and
resumable multi-source history bootstrap. It must not turn the seed into an
unlogged universal device-enrollment bearer.
