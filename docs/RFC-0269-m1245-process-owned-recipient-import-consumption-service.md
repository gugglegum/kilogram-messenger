# RFC-0269: Process-owned recipient import-consumption service core (M1.2.45)

Status: accepted as a verified service core; active runtime context attachment
remains fail closed.

## 1. One process, one bounded stage owner

`RecipientImportConsumptionRuntimeService` owns exactly one M1.2.41 response
dispatcher and one M1.2.42 process-local Plan cache. It decodes the canonical
nested request once, delegates Plan or Commit to those existing components and
returns only their current-Device-signed response bytes. It has no codec or
database for its opaque Plan stage, opens no network path and permits no silent
capacity eviction.

The service requires an already authenticated live recovery context supplied
for the current call: exact mutation adapter, intent store, current coordinator,
inbox/import stores, membership loader and current Device signer. It does not
infer missing consent or source authority from the request. That reconstruction
would be impossible without information deliberately excluded from the opaque
IPC contract and would create an unsafe authority shortcut.

## 2. Replay and lifetime rules

Before planning, the service checks the cache by the canonical Plan request
digest. An exact repeated request returns the original signed response byte for
byte without a second membership reload or a newly chosen `planned_at` value.
Commit requires the exact live same-process Plan summary. A fresh service after
restart therefore rejects Commit even if all durable state still exists.

Successful Commit retains the opaque Plan stage until its bounded expiry. This
is intentional: a caller may have lost the first response, and exact retry must
obtain the durable `AlreadyConsumed` receipt without a second consume or a
membership reload. Expiry removes the stage. Retaining it is not reusable
authority because every Commit is revalidated against the descriptor, current
Device, exact request and durable authorization/receipt state.

## 3. Active handler boundary

M1.2.45 does not invent a live recovery context in the ordinary runtime. The
M1.2.44 IPC v34 handler still returns the explicit unconfigured error until a
guided ceremony attaches an exact context for the actor lifetime. Consequently
the outer `RecipientImportConsumption` success variant remains unreachable in
the ordinary product process in this milestone.

The next stage must define that attachment lifecycle and then prove loopback
Plan/Commit/replay/restart behavior. CLI parsing, desktop controls, timers,
product network, public rollout and the `legacy-only` default remain unchanged.

## 4. Verification

A stable-path Rust regression constructs a real completed recovery fixture and
proves signed Plan, byte-exact Plan replay with one loader call, signed Commit,
durable `AlreadyConsumed` replay with zero loader calls, and rejection of Commit
by a restarted service that has no live Plan stage. No network, second host,
release build, archive, tag or automatic GitHub build is used.
