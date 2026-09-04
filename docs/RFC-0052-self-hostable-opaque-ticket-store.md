# RFC-0052: Self-hostable opaque ticket publication store

Status: M0.9.30 implemented (2026-09-05).

> Current protocol note: M0.9.33 / RFC-0055 adds mandatory unlinkable Ed25519
> write-capability headers. The historical unauthenticated PUT contract below
> describes M0.9.30, not the current executable.

## 1. Goal

RFC-0051 defined an authenticated, recipient-encrypted runtime ticket record but
used an in-test HTTP object. M0.9.30 supplies the first separately runnable
service for that wire contract: `kilogram-ticket-store`.

The service deliberately remains outside the account and conversation trust
model. It stores and returns opaque bytes. It has no dependency on Kilogram
identity, protocol, ratchet, runtime IPC, state vault, or transport crates and
does not decode the HPKE envelope. End clients remain responsible for all
confidentiality, signatures, authorization, expiry, and rollback checks.

This stage provides a self-hostable rendezvous object, not user discovery,
message storage, relay transport, push notifications, a blind mailbox, or
metadata anonymity.

## 2. Deployment boundary

The service implements bounded HTTP/1.1 and refuses every non-loopback listen
address. A production deployment is therefore:

```text
Internet client -- HTTPS --> operator reverse proxy -- loopback HTTP --> ticket store
```

TLS certificates, public ports, HTTP/2 or HTTP/3, access control at the edge,
and volumetric denial-of-service protection belong to the reverse proxy or
hosting platform. The store cannot accidentally be started as a cleartext
public listener. The client from RFC-0051 accepts remote services only through
HTTPS and permits HTTP only for a numeric loopback address.

The reverse proxy must buffer and bound requests, preserve the exact path, and
must not log request paths if the operator wants to avoid retaining lookup
channels. When `--trust-x-real-ip` is enabled, it must overwrite rather than
append or forward the incoming `X-Real-IP` header.

## 3. HTTP contract

The public paths remain:

```text
PUT /v1/ticket-publications/<64-lowercase-hex-channel>
GET /v1/ticket-publications/<64-lowercase-hex-channel>
GET /healthz
```

PUT requires:

```text
Content-Type: application/vnd.kilogram.ticket-publication
Content-Length: <bounded non-zero decimal>
X-Kilogram-Publication-Generation: <non-zero u64>
```

The service never compares the declared generation with fields inside the
opaque body. Successful first insertion returns 201, exact same-generation and
byte-identical replay returns 200, and replacement by a greater generation
returns 204. A lower generation or different bytes at the current generation
returns 409. Any higher generation is allowed so a publisher can recover after
failed uploads, migration between stores, or expiry of an earlier value.

GET returns 200 with the exact stored bytes, publication content type, clear
service generation and service expiry headers. A missing or expired value is
404. Unsupported methods are 405. Bodies never appear in error responses.

The monotonic header rule is a consistency aid, not publisher authentication.
Someone who learns a channel can race a high generation and deny future writes.
The recipient still rejects undecryptable or invalid content, but availability
against a channel-aware attacker requires a later write-capability design.

## 4. Durable opaque storage

Each channel is a 32-byte Redb key. Its value contains only:

- store schema version;
- client-declared generation;
- service receipt and expiry times;
- opaque request body.

No Account ID, Device ID, conversation label, contact name, ticket, endpoint,
device list, prekey, signature key, or decryption key is extracted or indexed.
Redb Immediate transactions make generation comparison and replacement atomic
and durable. The database is opened exclusively by one process; a second
instance using the same file fails instead of becoming another writer.

Reads use read-only transactions. An expired value is conditionally removed
under a write transaction, while a periodic bounded cleanup removes all other
expired or locally corrupt records. Cleanup runs at least once per minute and
also on startup.

## 5. Fixed retention

The operator configures one service-side retention interval for all records.
It must be between 30 seconds and one hour and defaults to 15 minutes. The
service ignores any encrypted client expiry because reading it would violate the
opaque boundary. A successful higher-generation replacement receives a new
service expiry. An identical retry does not extend retention.

Client cryptographic expiry and service retention are independent. The shorter
one wins in practice: the store can delete an otherwise valid publication, and
the client rejects a stored publication after its signed expiry.

## 6. Resource and parser limits

Defaults are intentionally explicit:

| Limit | Default | Hard configuration ceiling |
| --- | ---: | ---: |
| opaque body | 16 MiB | 16 MiB |
| live channels | 100,000 | 10,000,000 |
| combined opaque bodies | 1 GiB | 1 TiB |
| source-IP requests/minute | 120 | 10,000,000 |
| global requests/minute | 10,000 | 10,000,000 |
| concurrent connections | 128 | 4,096 |
| request headers | 16 KiB / 64 fields | fixed |
| request target | 512 bytes | fixed |
| connection I/O | 20 seconds | fixed |

HTTP parsing accepts one HTTP/1.1 request per connection, requires `Host`,
rejects duplicate headers, query/fragment targets, `Transfer-Encoding`,
`Expect`, request pipelining, oversized bodies, and bytes beyond the declared
body. Responses always close the connection and set `Cache-Control: no-store`
and `X-Content-Type-Options: nosniff`.

The connection semaphore bounds slow clients before allocating a task. The
global fixed-window limiter bounds total accepted work. The per-IP limiter uses
the TCP peer by default. Behind an explicitly trusted loopback proxy,
`--trust-x-real-ip` uses a single valid IP from the proxy-overwritten
`X-Real-IP` header. Invalid values fail closed.

These controls make resource use bounded but are not Sybil resistance,
bandwidth accounting, proof of work, or DDoS protection.

## 7. Operations

Local development:

```text
kilogram-ticket-store --data-dir <PRIVATE_DATA_DIRECTORY>
```

Production behind a correctly configured HTTPS reverse proxy:

```text
kilogram-ticket-store \
  --listen 127.0.0.1:8787 \
  --data-dir <PRIVATE_DATA_DIRECTORY> \
  --retention-seconds 900 \
  --max-record-bytes 16777216 \
  --max-channels 100000 \
  --max-total-bytes 1073741824 \
  --per-ip-requests-per-minute 120 \
  --global-requests-per-minute 10000 \
  --max-concurrent-connections 128 \
  --trust-x-real-ip
```

Startup prints the effective non-secret limits and the fact that reverse-proxy
HTTPS is required. It does not print channels, bodies, client IPs, or request
activity. Ctrl+C closes the listener and aborts remaining bounded connection
tasks. No Task Scheduler or background service registration is performed.

The two-network operator procedure is documented in
[`M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md`](M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md).

## 8. Verification

Regression covers:

- durable create, exact replay, same-generation conflict, rollback conflict,
  generation jump, channel capacity, process reopen, and exact TTL expiry;
- exact HTTP content type/path/generation behavior, body limit, trusted
  `X-Real-IP`, and deterministic 429 after the configured request count;
- complete RFC-0051 publish/fetch through this real service between two live
  runtime actors, including the idempotent recipient replay;
- refusal to bind a public cleartext address and strict lowercase channel
  parsing.

The release process smoke additionally starts the standalone executable and
observes HTTP `201/200/409/204` for create/replay/conflict/replacement. It then
terminates the process without graceful shutdown, restarts it against the same
Redb file, and confirms that generation 2 and the exact opaque bytes survive.
Rustfmt, strict workspace Clippy, all 193 workspace tests, the release workspace
build, and the release-mode two-actor runtime lifecycle pass.

## 9. Honest boundary and next stage

The operator still observes public client IPs at the reverse proxy, lookup
channels, timing, frequency, body sizes, declared generations, and retention
events. Redb is not encrypted by this service because payload confidentiality
already comes from recipient HPKE; disk encryption and access controls remain
operator responsibilities for metadata protection.

M0.9.30 does not authenticate writers, hide reads, replicate the database, give
availability guarantees, or refresh publications automatically. The next
bounded stage is an opt-in runtime policy for scheduled publish/refresh with
clear foreground/background, network-class, expiry, retry/backoff, and user
visibility rules. It must not silently turn every client into a relay or daemon.
