# RFC-0186: M1.1.60 dormant bounded legacy-history import

Status: accepted locally; migration primitive verified, active runtime unchanged.

## Outcome

`kilogram-store` now provides a deterministic import plan and a bounded page
operation from the installed filesystem `EventStore` into the separate
namespace-aware `CanonicalSyncV2CarrierStore`. The operation never translates
an installed event into a native canonical event. It wraps the exact verified
`AuthorizedEvent` Postcard bytes in the already-defined legacy carrier and
therefore preserves the original author, certificate and authority signatures.

An import plan binds:

- the exact conversation and owner-signed membership snapshot;
- the snapshot revision and exact verified Postcard bytes;
- every source Event ID and the exact verified authorization bytes;
- strict Event-ID ordering and a maximum inventory of 4096 events.

These values produce a domain-separated BLAKE3 plan ID. This is a local
integrity/freshness claim, not a new network signature or wire identifier.
Rebuilding a plan after process restart produces the same ID only while the
selected source snapshot is byte-identical.

## Bounded resume

One call imports between 1 and 64 events, matching the closed `/cbor/2` batch
bound. The returned cursor contains the exact plan ID and next source index.
Retrying the same cursor is idempotent because the target carrier bytes and
typed storage key are immutable.

A caller may reconstruct a cursor after restart, but it cannot use that cursor
to skip work: every prefix before `next_index` must already exist in the target,
strictly decode as a legacy carrier, match the plan's exact source digest and
remain authorized by the exact membership snapshot. The new page is validated
from the source and committed through the existing single redb transaction.
One conflict leaves every previously missing item in that page absent.

Completion is relative to the immutable plan. Events appended after plan
creation require a new plan; already imported exact carriers make restarting
that larger plan idempotent.

## Deliberate transaction boundary

The cursor is not durably stored in this stage. Persisting `next_index` in an
ordinary file before or after the carrier transaction would create a crash
window and a false atomicity claim. The next slice must store the plan ID and
cursor in the same redb transaction as the imported page, then couple that
local migration state to the signed `/cbor/2` session checkpoint.

## Non-activation

No CLI command, Iroh endpoint, active wire path or automatic startup invokes
the importer. `/cbor/2` remains unregistered and undialed. The active Postcard
store remains authoritative, and deleting or replacing it is outside this RFC.

## Remaining activation work

Activation remains blocked on:

1. atomic durable import cursor plus typed-carrier transaction;
2. durable signed sync-checkpoint runtime integration;
3. mutual authenticated capability selection with no error fallback;
4. mixed-version, downgrade and rollback regression;
5. sustained fuzzing and fresh hosted plus physical-device evidence.
