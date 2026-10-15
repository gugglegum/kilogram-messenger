# RFC-0166: M1.1.40 canonical own-device announcement automation

Status: implemented as an inactive M1.1.40 candidate.

## 1. Scope

M1.1.40 specifies the local IPC bytes used to configure and inspect automatic
publication of this Device's current endpoint announcement to one other active
Device in the same Account. It deliberately covers only the announcement
scheduler. Discovery of a sibling's current ticket and orchestration across the
complete roster remain later commands.

The active runtime still uses Postcard IPC version 26 with JSON descriptor
version 7. This RFC does not activate canonical CBOR, does not change the
desktop/CLI adapter, and executes no network operation.

## 2. Exact authority and ticket anchors

Both commands embed the exact current Root-signed Account Device list. The
descriptor Account and local Device must be present in that list. A configured
recipient must be a different active Device in the same exact list.

The recipient connection ticket stays in a trusted content-addressed staging area.
IPC carries only its bounded byte length and a BLAKE3 digest under the
dedicated `own-device-announcement-artifact-digest` domain. Recipient ticket
plaintext, local filesystem paths, Root secrets and publication write
capabilities are forbidden from the request and response.

## 3. Commands

`configure-own-device-announcement-automation` includes:

- the exact Root-signed current Device list;
- either no predecessor for generation one or the exact expected prior
  generation and Policy ID;
- the recipient Device ID and content-addressed ticket reference;
- enabled state, interval, ticket validity and bounded retry policy; and
- an explicit four-bit Ethernet, Wi-Fi, mobile and unknown network mask.

`own-device-announcement-automation-status` includes only the exact current
Device list. It cannot silently project policy state under a different Account
authority revision.

The configure request ID is the durable idempotency key. An exact retry may
return `already-committed`; reuse with different bytes or a stale predecessor
must fail before policy mutation.

## 4. Policy chain and scheduler semantics

Policy generation one has no predecessor. Every later generation advances
exactly one step from the exact prior Policy ID. The Policy ID commits the
Account ID, local Device ID, authority revision, Device-list digest, generation,
predecessor, configuration time and complete configuration.

The signed scheduler state is derived rather than trusted independently:

- disabled configuration reports `disabled`;
- an enabled policy whose current network bit is denied reports
  `network-blocked`;
- a permitted policy with failures reports `backoff`; and
- otherwise it reports `scheduled`.

Attempt, success, next-attempt, bundle and transport option tuples must be
nonzero when present, presence-consistent and temporally ordered. A successful
attempt binds both a nonzero Bundle ID and direct/relay transport.

## 5. Bounded status snapshot

Status contains at most 32 policies in strict recipient Device-ID order. Every
recipient must still be active in the supplied exact Device list and must not be
the local Device. The Snapshot ID commits the Account, local Device, exact
authority state, current network and the complete ordered policy projection.

Both configure and status results are Device-signed and bind the complete
request digest. They explicitly report:

- no runtime restart is required;
- execution belongs to the foreground runtime only; and
- no OS background service is enabled.

This means automation runs while Kilogram is running. It does not register a
Windows Task Scheduler task or promise delivery while the application is
closed.

## 6. Limits and failure behavior

The request and response limits are 768 KiB and 512 KiB. A staged recipient
ticket is at most 8 MiB. Intervals range from 30 seconds through 24 hours,
validity through 30 days, and retry delays through one hour.

Unknown variants, noncanonical CBOR, trailing bytes, invalid signatures,
authority substitutions, stale predecessor claims, invalid scheduler tuples,
path or plaintext leakage claims and an enabled OS background service all fail
closed.

## 7. Evidence and activation boundary

The Rust codec and contextual verifier are covered by deterministic golden
vectors, an independent Node.js implementation using platform Ed25519 and the
independent repository BLAKE3 reference, and the bounded mutation corpus. The
new boundary also proves that active Postcard IPC version 26 and JSON
descriptor version 7 are unchanged.

Activation still requires the remaining runtime IPC commands, an explicit IPC
version allocation and migration, dual-reader/downgrade tests, sustained
coverage-guided fuzzing and fresh desktop/CLI field evidence.
