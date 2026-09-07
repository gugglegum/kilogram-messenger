# RFC-0082: M1 acceptance kit and trusted HTTPS mailbox preflight (M0.9.60)

Status: implemented; the real Alice/Bob field run remains an explicit operator
action.

## 1. Problem

M0.9.58 defines a machine-verifiable mailbox lifecycle field test, but its
individual commands are easy to transpose and it assumes that a public HTTPS
mailbox service is already usable. A listener printing `status=listening` on
Alice does not prove that Bob can reach the service through TLS, that Windows
trusts its certificate, or that the configured store signing key matches the
service which actually answered.

The M1 acceptance operation also must not restore the expensive everyday
release/ZIP loop, copy private state into a synchronized directory, or create
another executable with a changing filename and a new Windows Firewall
identity.

## 2. What the mailbox store is

The store is a deliberately limited availability helper. A sender uploads a
bounded recipient-encrypted envelope and receives a store-signed acceptance
receipt. The recipient later downloads, authenticates and decrypts the envelope,
commits the event locally, and only then conditionally deletes the stored item.

The store never receives message plaintext, Account/Device/conversation IDs or
the recipient read capability through its application contract. It still sees
network metadata such as source IP, timing, byte counts and repeated access to
an opaque mailbox. Its signed receipt proves acceptance, not continued
availability; the service can still lose, delay or suppress data.

`kilogram-ticket-store` is a Rust executable and is not tied to an operating
system. Linux is convenient for a permanent VPS deployment. Windows is valid
for the controlled test. It intentionally binds only to loopback HTTP and must
remain behind an HTTPS reverse proxy such as Caddy or Nginx.

## 3. Store on Alice

For the controlled field test, the store may run as an additional process on
the Alice computer. During the offline-recipient phase only Alice's messenger
runtime and desktop client are stopped. The Alice computer, store process and
HTTPS reverse proxy remain running, so Bob can place an encrypted envelope.

This topology proves asynchronous client delivery but does **not** prove
delivery while the entire Alice computer is powered off. That case needs a
separate always-on VPS/store or another authorized always-on peer. If Bob is
outside Alice's LAN, the HTTPS name and TCP 443 must really reach Alice; public
DNS plus port forwarding and the absence of carrier-grade NAT are operational
prerequisites.

The loopback-only store invariant is not relaxed for convenience. The reverse
proxy terminates TLS and forwards locally to `127.0.0.1:8787`. The acceptance
preflight uses the ordinary Windows certificate store; it has no certificate
bypass and follows no redirects.

## 4. Portable acceptance kit

`scripts/new-kilogram-m1-acceptance-kit.ps1` is an explicit, no-clobber kit
builder. It requires a clean Git HEAD, runs the existing network-free static
mailbox gates, then builds only the debug `kilogram-cli` and
`kilogram-windows` targets with bounded Cargo parallelism. It does not create a
release build or ZIP and it does not launch a Kilogram network process.

The directory contains:

- stable `bin/kilogram-cli.exe` and `bin/kilogram-windows.exe` names;
- `BUILD-INFO.json` with the exact commit, size and SHA-256 of both EXEs;
- the precomputed `BOUNDARIES.log`;
- `RUN.ps1`, the ordered step driver and existing evidence helpers;
- local-config and evidence-manifest examples;
- the Russian operator procedure.

Every step rechecks both executable hashes before doing work. A copied or
partially synchronized binary therefore fails before it can be launched.

## 5. Private-state boundary

Actual `.psd1` role configs are local pointers and must be created outside the
shared kit and evidence directory. Runtime profile, bearer-bearing IPC
descriptor, state directory, seed and device/vault keys are never copied by the
kit builder or driver. Relative paths in a role config resolve against that
config's private local directory.

The shared evidence directory contains only the versioned public field records
defined by M0.9.58. All writes are no-clobber. A failed or repeated experiment
uses a new evidence directory and run ID.

## 6. HTTPS preflight

`test-kilogram-mailbox-store-preflight.ps1` runs on Bob before phase 01 and
validates:

1. an absolute HTTPS URL with no credentials, query or fragment;
2. a bounded regular startup log with `opaque-redb-v1`, both reverse-proxy HTTPS
   markers and the exact pinned 32-byte store public key;
3. absence of application identifiers in the supplied service startup output;
4. `GET /healthz` over the default Windows TLS trust path, with redirects
   disabled, a bounded response and exact `ok\n` body.

The preflight is reachability and identity-configuration evidence, not a proof
that a remote operator does not keep infrastructure logs. Its network-free
self-test accepts a coherent synthetic setup and rejects a changed key and
remote cleartext HTTP.

Alice may copy the bounded startup stdout to a shared staging location for this
step: it contains the public store key and limits, not a mailbox read/write
capability. Bob's config points to that copy. The driver verifies it and writes
the canonical `05-store.log` evidence. Private runtime profile, IPC and state
remain local and are never shared.

## 7. Completion boundary

M0.9.60 implements and statically verifies the kit, step inventory and HTTPS
preflight without building the kit or opening the network. M1 field acceptance
is complete only after operators generate the kit deliberately, execute the
M0.9.58 lifecycle on Alice and Bob, and the final evidence verifier reports
`result : verified`.

The M0.9.59 GitHub run for commit
`e84d557dd80e07721296777aebe8ebbc6a8af392` is recorded as a useful but
non-blocking independent-build result: the local and hosted binaries differed
by 1024 bytes while using MSVC linker versions 14.44 and 14.51 respectively.
No attestation was issued. Byte-identical cross-environment reproducibility is
deferred to the public-release hardening track and is not an M1 messenger
acceptance requirement.
