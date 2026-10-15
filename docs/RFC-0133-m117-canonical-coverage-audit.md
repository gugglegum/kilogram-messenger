# RFC-0133: Canonical schema coverage audit (M1.1.7)

Status: accepted as an inactive coverage baseline.

## 1. Purpose

`CANONICAL-COVERAGE.json` joins the closed 20-surface compatibility registry to
the canonical artifacts produced so far. It prevents a handful of good vectors
from being mistaken for complete protocol coverage.

Coverage is `complete` only when every object, cryptographic preimage and
semantic relation in one source surface has a canonical schema, strict decoder,
golden vector and independent reference. `partial` always carries an explicit
remaining list. `not-started` means no target artifact exists. `not-targeted`
is reserved for explicit non-Postcard codecs outside this migration.

## 2. Result

Of 20 registered surfaces, two are complete, four are partial, twelve are not
started and two are deliberately not targeted. Device-session authorization is
complete with its nested certificate, authority snapshot and revoked-device
check. M1.1.9 also completes account authority with the Root-signed device list,
conversation membership and request-bound device-link authorization.

Existing JSON connection tickets and the JSON local desktop bootstrap contract
are outside the Postcard-to-CBOR migration. This is not a claim that JSON is
the final public protocol for all time; changing either remains an explicit
versioned migration under `PROTOCOL-COMPATIBILITY.json`.

## 3. Priority

The next work first completes the central event and sync surfaces, followed by
the remaining public ratchet and recovery objects. Mailbox
capabilities and transport follow because offline delivery depends on them.
Runtime IPC can migrate independently because it is a local-control surface.

## 4. Security boundary

The audit changes no active wire bytes. A partial surface is not activation
ready, even when its covered subset has strict decoding and valid signatures.
The aggregate `activation_ready` flag remains false until every targeted
surface is complete and the separate migration requirements are satisfied.
