# RFC-0268: Active runtime IPC recipient-import-consumption envelope (M1.2.44)

Status: accepted as an active local-control envelope with a fail-closed,
unconfigured handler.

## 1. Atomic local-control migration

M1.2.44 performs the migration preflighted by M1.2.43 as one change-set. The
Postcard runtime IPC advances from version 33 to version 34 and the signed JSON
descriptor domain advances from `kilogram:runtime-ipc-descriptor:v14\0` to
`kilogram:runtime-ipc-descriptor:v15\0`. The command and response enums append
exactly these variants:

- `RecipientImportConsumption { request: Vec<u8> }`;
- `RecipientImportConsumption { signed_response: Vec<u8> }`.

The byte vectors are opaque carriers for the independently canonical and
signed recipient import-consumption contract. The outer Postcard envelope does
not parse, reinterpret, authorize or mutate that nested request.

## 2. Exact-version downgrade behavior

This is a hard cut for co-installed local binaries, not a dual reader. A
version 33 descriptor, including one correctly signed under descriptor domain
v14, is rejected before connect. An authenticated version 33 request is
rejected before actor dispatch. Mixed installations must update both binaries
and restart the runtime; there is no format sniffing, fallback or silent
downgrade. Hybrid 33/v15 and 34/v14 states are invalid.

## 3. Registered does not mean operational

The runtime actor recognizes the new command so the active enum is exhaustive,
but M1.2.44 does not yet give that actor ownership of the M1.2.42 process-local
plan-session cache or the M1.2.41 response dispatcher. It therefore returns the
explicit bounded error `recipient import-consumption runtime service is not
configured`.

The success response is registered in the byte contract but is unreachable in
this milestone. No unsigned outcome, mutation success or implied Commit
authority can escape through the placeholder handler. Wiring cache ownership,
expiry/capacity behavior and signed response dispatch is the separate M1.2.45
stage.

## 4. Scope and verification

The canonical completion inventory, schema, CDDL, compatibility registry and
all live binding gates move atomically to 34/v15. Historical manifests retain
their original claims and recognize this explicit successor rather than being
rewritten.

The dedicated boundary rejects ten contract mutations. Stable-path Rust
regressions cover both the v33 request and v14-signed descriptor downgrade
paths. The full readiness inventory contains 170 gates. No user-facing CLI or
desktop ceremony, peer/relay/mailbox/sync wire, product network, release build,
archive, tag, automatic GitHub build or second host is part of this stage. The
public default remains `legacy-only`.
