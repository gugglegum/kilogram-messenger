# RFC-0170: M1.1.44 canonical history-recovery execution and scheduler state

Status: implemented as an inactive M1.1.44 candidate.

## 1. Scope

M1.1.44 specifies deterministic-CBOR requests and Device-signed responses for
starting and observing execution of an already approved history-recovery plan.
It also specifies the recipient-Device-signed, append-only scheduler state that
survives a foreground runtime restart and binds every attempt lease, retry,
progress checkpoint, cancellation and completion transition.

This slice does not activate canonical CBOR, change Postcard IPC version 26 or
register an OS background service. It does not let a GUI manufacture network
success: attempt transitions are runtime-produced signed state, while routine
IPC only starts a plan or reads its current snapshot.

## 2. Start and status commands

`start-history-recovery-execution` names a nonzero plan ID and the exact
content-addressed approved plan record. It fixes bounded retry base, retry
maximum and attempt-lease durations. The expected record comparison prevents a
stale caller from starting a cancelled or concurrently changed plan. The
request ID is the durable idempotency key: an exact retry may return
`already-committed`, while divergent reuse fails before scheduler mutation.

`history-recovery-execution-status` names only the plan ID. The response embeds
the current plan record, signed scheduler head and a runtime-observed execution
snapshot. Neither command accepts network class, power state, checkpoint or
attempt outcome from the caller.

## 3. Signed scheduler chain

Generation zero is an `initialized`/`active` state and contains a freshly
recipient-signed initial canonical recovery checkpoint. Every successor binds
the exact previous scheduler-state ID and increments generation by one. The
chain has six transitions:

- `attempt-started` moves `active` to `attempting`, increments the total attempt
  counter and establishes a bounded lease;
- `attempt-failed` returns to `active`, increments consecutive failures and
  applies deterministic equal-jitter exponential backoff;
- `attempt-progressed` returns to `active`, clears consecutive failures and
  commits an exact valid successor recovery checkpoint;
- `completed` commits a terminal checkpoint at the approved range end;
- `cancelled` mirrors the immutable cancellation tombstone in current plan
  custody;
- `initialized` is valid only at generation zero.

Retry configuration and the attempt lease are signed into every state, so a
restart cannot silently reinterpret the chain. Progress and completion carry
the complete signed checkpoint rather than a caller-provided count or boolean.
Failed attempts carry a nonzero bounded failure code but no remote error text.
Counter overflow, a zero failure code and any retry delay other than the exact
derived equal-jitter value fail closed during successor verification, including
for a correctly signed state received from another implementation.

## 4. Runtime-observed readiness

Each response snapshot records observation time, network class and power class
inside the Device-signed response. Readiness is derived, never accepted as an
independent claim. Terminal cancellation or completion wins first, followed by
plan expiry, clock rollback and attempt-lease state. For an active plan the
exact plan policy then yields `blocked-network`, `blocked-power`, `deferred` or
`ready`.

Unknown network is allowed only when the approved plan explicitly permits it.
Unknown or battery power does not satisfy `require-external-power`. A
`ready` snapshot means only that a bounded attempt may begin; it is not a claim
that a source is online or that history is available.

## 5. Privacy and execution boundary

The surface contains public signed authorization artifacts, retry metadata and
opaque checkpoint commitments. It excludes Account Root secrets, Device
private keys, recovery capabilities, decrypted history, page payloads, remote
deletion commands and local filesystem paths. Every successful snapshot states
`restart-not-required`, `foreground-runtime-only` and
`os-background-service-enabled=false`.

Closing the messenger therefore stops new attempts. Scheduling is durable, but
not an operating-system task. Multi-source orchestration remains a higher-level
composition of independently approved source plans; this contract does not
pretend that one plan transfers from several sources.

## 6. Verification and activation

Rust round trips and transition substitution tests, checked-in golden vectors,
an independent Node implementation and the bounded decoder mutation corpus
protect this candidate. The active Postcard runtime, desktop and CLI remain
unchanged. Activation still requires the explicit IPC migration, dual readers,
coverage-guided fuzzing and fresh field evidence listed by the aggregate
runtime IPC schema.
