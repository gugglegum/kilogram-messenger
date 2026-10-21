# RFC-0272: Runtime-owned explicit guided session controller (M1.2.48)

Status: accepted as a dormant, process-owned orchestration boundary. It is not
registered with runtime IPC, CLI, desktop, timers, or product networking.

## 1. One controller, one explicit session

M1.2.48 combines the existing read-only import Plan, Device-signed durable
confirmation, bounded durable mutation, M1.2.47 authenticated completion and
M1.2.46 consumption actor in one process-owned controller. Its explicit phases
are `Idle`, `Planned`, `Confirmed`, and `Attached`, and no more than one guided
session may be active.

The controller starts idle and owns its state-directory binding, Account,
already loaded Device signer reference, durable adapters and one consumption
actor. Session calls do not accept a path, signer secret, coordinator object or
membership snapshot. `begin` consumes an
`AuthenticatedGuidedRecipientImportSession`: a process-local Rust value with
private fields, no public constructor, and no Clone, Debug, Serialize or
Deserialize implementation. The token exact-binds state, Account, Device,
signed runtime descriptor, Plan request, coordinator and ordered live
memberships before the controller sees it.

## 2. Explicit phases and authority custody

`begin` reruns the canonical read adapter and retains its ephemeral Plan only
inside the controller. `confirm` requires the exact session ID and explicit
plan confirmation phrase, then uses the current Device signer and existing
append-only confirmation ledger. `apply` requires the same session ID, consumes
the durable confirmation, and invokes the existing bounded mutation adapter.

Only a fresh complete mutation carrying the M1.2.47 non-serializable completion
may install the M1.2.46 context. A rejected or partial mutation, or a loader-free
completed replay without a fresh completion, cannot attach the actor. Once
attached, explicit consumption bytes are delegated to the existing v34 actor;
the controller supplies the retained descriptor and Device signer instead of
accepting either from the session caller.

## 3. Fail-closed lifecycle

Confirmation and mutation take ownership of the current ephemeral phase before
performing any fallible operation. Therefore a bad phrase, wrong session ID,
expiry, stale durable state, rejected mutation or missing completion leaves the
controller idle and the actor unattached. A durable confirmation may remain as
evidence of a previous explicit approval, but it cannot reconstruct session
authority.

Exact-ID cancel drops Planned or Confirmed state and detaches Attached state.
Expiry does the same and clears the bounded consumption Plan cache. A new
controller after process restart is idle even when all durable confirmation,
mutation and import ledgers still exist. The session lifetime is at most 300
seconds. Foreign state-directory, Account and Device tokens are rejected before
planning.

## 4. Verification and product boundary

One real recovery fixture drives `Idle -> Planned -> Confirmed -> Attached`,
then exercises signed consumption Plan, byte-exact replay, Commit, durable
`AlreadyConsumed`, attached expiry and restart. A second fixture proves
single-session exclusivity, Planned and Confirmed cancellation, expiry,
confirmation and mutation failure cleanup, and foreign state/Account/Device
rejection.

No product network is opened and no second host is required. Peer wire and
runtime IPC remain unchanged. Ordinary startup has no token producer and stays
idle/unattached; the public sync default remains `legacy-only`. A later stage
may add a local UX-facing producer, but must not expose the token's authority
fragments or create ambient/background recovery.
