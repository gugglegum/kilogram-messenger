# RFC-0270: Guided recipient import-consumption runtime attachment (M1.2.46)

Status: accepted as a process-local attachment boundary; no product ceremony
installs the context yet.

## 1. Runtime-owned actor

The ordinary runtime now owns exactly one
`RecipientImportConsumptionRuntimeActor` for its whole process lifetime. The
actor owns the single M1.2.45 service and one optional guided-context slot, and
the active IPC v34 handler delegates the registered opaque Plan/Commit envelope
to that actor. It does not construct a new cache or dispatcher per request.

The runtime deliberately starts with the slot unattached. In that state the
same explicit error remains observable:
`recipient import-consumption runtime service is not configured`. A signed
success is therefore still unreachable in the ordinary product flow until a
separate guided recovery ceremony supplies the live context in-process.

## 2. Capability attachment, not ambient configuration

`GuidedRecipientImportConsumptionContext::from_authenticated_recovery` accepts
the already authenticated recovery coordinator and exact membership snapshots.
It binds them to the state directory, Account, Device, coordinator plan and
sequence, ordered membership references and their canonical digests. The
derived attachment identifier also commits to installation and expiry time.

The value is process-local, non-durable and bounded to at most 300 seconds. It
cannot be supplied by the outer runtime IPC payload, an environment variable or
a filesystem scan. Installing a different context while one is live is
rejected; an exact reinstall is an idempotent replay. Explicit clear, expiry and
process restart destroy both the context and every opaque Plan stage. This
prevents a Plan prepared under one recovery authority from being committed
under another.

## 3. Dispatch and mutation boundary

Every dispatch rechecks the descriptor Account and Device and the live signer
Device before the M1.2.45 service decodes and validates the nested request. The
membership loader can return only exact snapshots held by the attached
capability. The actor reports state change only for a newly consumed import;
Plan and durable `AlreadyConsumed` replay remain read-only from the runtime
notification perspective.

The attachment opens no network path and adds no CLI command, desktop control
or timer. It changes neither peer wire nor runtime IPC wire, and the public
default remains `legacy-only`.

## 4. Verification

A stable-name Windows regression uses a real durable recovery fixture to prove
unattached rejection, Plan and byte-exact Plan replay, exact-ID clear, stale
Commit rejection, reinstall without resurrection of an old Plan, fresh
Plan/Commit consumption, durable `AlreadyConsumed` replay, expiry cleanup and
an unattached restarted actor. The boundary verifier also proves that the
active runtime creates one actor and has no call that installs a context.

No network, second host, release build, archive, tag or automatic GitHub build
is used in this milestone. M1.2.47 may connect a producer only from the existing
authenticated guided-recovery ceremony and must then prove the loopback IPC
success path without introducing ambient authority.
