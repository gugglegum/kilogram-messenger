# RFC-0266: bounded recipient-import consumption Plan session cache

Status: implemented and verified in M1.2.42.

## Why this is a separate milestone

M1.2.41 can produce a signed read-only Plan and can commit only when handed the
opaque result of that exact Plan call. A real long-running runtime therefore
needs temporary custody for that opaque value between two authenticated IPC
requests. Reconstructing it from the signed response would turn a visible
projection into mutation authority, while persisting it would make stale Plan
stages survive process restart.

Adding an active command is a distinct compatibility migration. The active
Postcard enum remains version 33 / descriptor domain v14 in this milestone.
Registering a new command will require the explicit version 34 / domain v15
hard cut for co-installed runtime, CLI and desktop binaries. M1.2.42 prepares
the authority boundary without hiding that future wire change.

## Cache contract

`RecipientImportConsumptionPlanSessionCache` owns at most 32 live opaque
M1.2.41 dispatcher stages. It is a plain process-local Rust value: it has no
codec, database, file, background worker or network surface. Restart constructs
an empty cache and therefore requires a fresh read-only Plan.

Insertion accepts only a successful, non-expired `Planned` dispatcher result.
An exact replay of the same plan ID, summary and Plan request digest is
idempotent. A collision fails closed. When live capacity is exhausted, the
cache rejects the new stage rather than silently evicting another unexpired
ceremony. Expired stages are purged before lookup or insertion.

The cache does not grant Commit authority. Commit must name the exact cached
plan ID and byte-equivalent plan summary. It then calls only the M1.2.41
dispatcher, which revalidates the request against the current runtime
descriptor and Device, reloads live durable recovery state, commits explicit
authorization before mutation, and signs success only after the durable
consumption receipt exists. Same-process exact replay remains available until
expiry and resolves through the durable authorization and receipt without a
second intent consumption.

## Failure and privacy properties

- stage skipping and Commit after restart fail closed;
- expiry is evaluated before a cached stage can be used;
- capacity pressure cannot silently transfer or replace authority;
- no Account, Device, membership, history or secret is written by the cache;
- Plan remains read-only and creates no durable cache state;
- no active runtime command, CLI/desktop control or network path is added;
- connection-ticket, sync and history-transfer wire formats do not change;
- the public default remains `legacy-only`.

## Verification

The durable fixture proves invalid capacity rejection, exact Plan replay,
restart loss, same-process Commit, durable `AlreadyConsumed` replay, expiry and
zero network authority. Windows runs it through the stable-path harness. The
formal boundary includes 10 fail-closed mutations, and aggregate M1 readiness
contains 168 closed gates. No release build, ZIP, tag, automatic GitHub build
or second host is involved.
