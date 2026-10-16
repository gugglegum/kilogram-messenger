# RFC-0176: M1.1.50 canonical sync ALPN allocation

Status: accepted locally; identifier allocated, not active.

## Outcome

M1.1.50 allocates the exact parallel ALPN
`kilogram/m1/sync/cbor/1` for the inactive
`kilogram-deterministic-cbor-v1` synchronization candidate. The active ALPN
remains `kilogram/m0/sync/8`, and the active payload remains Postcard v6.

The transport library now owns a closed `SyncWireProtocol` registry and an
exact fail-closed classifier. Unknown, truncated and future identifiers are
rejected. Format sniffing and parse-error fallback are forbidden.

## Why allocation is not activation

Iroh negotiates ALPN before Kilogram reads a session authorization frame.
Merely teaching both peers the new byte string would therefore not prove that
the remote Device is currently authorized or that it supports the canonical
codec under the current Account authority. An unauthenticated hint must never
select canonical CBOR.

Consequently the new identifier is deliberately absent from every active
endpoint ALPN list and every dial path. The CLI still registers and dials only
`kilogram/m0/sync/8` for synchronization. This stage changes no active wire
bytes and executes no network operation.

## Required selection rule

Before a future initiator may dial the canonical ALPN, it must possess a fresh
Device-signed listener capability bound to the current authority state. After
the canonical connection opens, the requester must still authenticate with the
canonical Device session authorization before any synchronization frame is
accepted.

Once the canonical ALPN is selected, a connection failure, unknown frame or
decode failure fails closed. It must not silently retry the same operation over
the legacy ALPN. Rollback is an explicit local or release policy transition,
never a response to a parse or network error.

## Remaining work

Activation remains blocked on authenticated capability advertisement, the
active dual reader and writer, mixed-version and downgrade regression tests, a
rollback drill, sustained fuzzing, and fresh hosted plus physical-device
evidence. Allocation alone is not dual-reader support and is not activation.
