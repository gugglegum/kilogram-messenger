# RFC-0211: M1.1.87 canonical sync v2 runtime observability

Status: accepted locally.

## Decision

The foreground runtime exposes one authenticated, read-only local IPC snapshot
that distinguishes the canonical sync v2 policy loaded when the process built
its Iroh endpoint from the durable policy configured now. The Windows desktop
shows both projections, the exact six transport permissions and an explicit
`restart required` state when the projections differ.

The same snapshot includes a bounded process-local trail of sync paths actually
observed by this runtime. It identifies inbound or outbound direction, legacy
Postcard v6 or canonical CBOR v2, outcome, transport endpoint identity and
available authenticated Device, conversation and selection identifiers. It
does not infer canonical use from a configured policy: a row appears only after
the corresponding runtime path was observed.

The public default remains `legacy-only`. This stage adds no activation
mutation, hot apply, endpoint rebind, peer wire change or sync wire change.

## Loaded policy versus configured policy

The loaded projection is captured from the same verified Device-bound policy
used to construct the endpoint ALPN list at startup. It cannot change during
the process lifetime. For every status request, the runtime obtains the
existing state-directory lock and verifies the current durable activation
chain again. Exact equality means the running endpoint matches disk;
inequality means a fresh runtime is required.

An absent activation chain is displayed as the implicit `legacy-only` default.
A malformed, foreign, forked or rolled-back chain remains a hard error; it is
not converted into a permissive status. The read command does not repair or
append authority state.

## Bounded path observations

Observations exist only in process memory. The runtime keeps at most 32 rows,
drops the oldest row on overflow and assigns a process-local monotonic sequence
number. Restart deliberately clears the trail. The desktop presents only the
eight newest rows, while the typed IPC response remains bounded to 32.

The projection may contain public transport endpoint identity and already
authenticated Device, conversation and selection IDs. It cannot contain
message plaintext, ciphertext payloads, IP addresses, relay URLs, filesystem
paths, bearer tokens, Device or Account Root secrets, or mailbox capabilities.
It is operational observability, not a durable audit log.

## Local IPC migration

Adding a Postcard enum variant changes discriminants, so co-installed runtime,
CLI and desktop binaries move together from exact IPC v29 / descriptor domain
v10 to exact IPC v30 / descriptor domain v11. Correctly signed v29 descriptors
and v29 requests fail before actor dispatch. There is no local IPC dual reader
or silent downgrade.

The canonical target inventory adds one read command,
`canonical-sync-v2-runtime-status`, and its bounded result. Canonical CBOR is
still not activated by this bookkeeping update.

## Desktop behavior

An authenticated connect loads the snapshot alongside existing directory
status. The advanced activation panel can refresh it explicitly, and the
existing runtime change-notification chain refreshes it after other projections.
The stopped-runtime status/plan/apply ceremony from M1.1.86 remains the only UI
path that can request a policy transition. It still requires disconnection,
exact planning, typed confirmation and a subsequent fresh runtime.

## Verification and non-claims

Unit coverage checks the 32-row ring, monotonic sequence eviction, implicit
default and exact restart divergence. Runtime IPC regression rejects v29 before
dispatch. Desktop tests and a dedicated fail-closed boundary pin the read-only
command, projection limits, forbidden data, v30/v11 identity and unchanged
`legacy-only` default.

Readiness contains 114 closed gates; rustfmt and locked/offline strict Clippy
pass. No network run, release build, archive, tag or automatic GitHub build is
part of this stage. This RFC does not make observations persistent, enable a
public dual-reader default, materialize canonical-native history, retire legacy
sync or replace an independent security audit.
