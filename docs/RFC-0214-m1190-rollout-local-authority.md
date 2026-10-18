# RFC-0214: M1.1.90 dormant rollout local authority

Status: accepted locally; implementation is dormant and public default remains
`legacy-only`.

## Decision

Kilogram now has the two local primitives that M1.1.89 deliberately left as
engineering prerequisites: a privacy-preserving installation cohort authority
and a bounded durable rollout-health aggregate. They live in the separate
`kilogram-rollout` crate. No runtime, desktop, CLI, transport or protocol crate
depends on it, so the implementation cannot enroll a user or change active wire
behaviour merely by being compiled.

This is not permission to begin a public canary. The N/N-1 executable matrix,
an operational containment rehearsal and an independent security review remain
open. Cohort selection also returns `runtime_activation_authorized=false`; a
later integration must still pass through the existing Device-signed activation
authority and exact fresh-process verification.

## Signed release policy

A release policy is a bounded canonical Postcard document signed with Ed25519.
Verification always receives an explicitly pinned release public key. The
local database pins that key on first open and refuses a different key later.
Kilogram does not embed a test or production release key in this stage, and no
policy download or distribution channel is added.

The first policy must be epoch one, `legacy-only-hold`, normal disposition and
zero cohort. Every successor must name the exact previous signed-policy ID and
increment the epoch by one. A phase can stay unchanged or advance by exactly
one step; it cannot skip or rewind. Containment preserves the current phase and
sets enrollment to zero rather than erasing canonical state. The policy model
stops at `expanded-dual-reader`: broad default is intentionally not
representable without another design and release decision.

The signed-policy ID is SHA-256 over a domain and the exact canonical signed
bytes. It is release-wide, not installation-specific, and therefore is not a
user identifier.

## Installation-local cohort

On first open, the local store creates a 32-byte operating-system CSPRNG seed
inside `rollout-local-v1.redb`. The public API never returns it. Assignment is
exactly `SHA256(seed || signed-policy-id)`, taking the first big-endian `u64`
modulo 10,000. A checked vector fixes this interpretation.

No Account root, Device ID, contact, conversation, address, relay or path is an
input. Different devices of one account are independent; reinstalling may
produce a different assignment. The store exposes no synchronization or
network API. A later application integration is responsible for placing this
database in an installation-local, non-roaming directory.

The returned evaluation includes the local bucket for in-process decision
making, but an exported health document omits both seed and bucket. Hold and
containment never select an installation. Each phase enforces the maximum from
M1.1.89: 1%, 1%, 5% and 25% respectively.

## Bounded durable health evidence

The same redb database retains at most sixteen fixed-schema aggregates, keyed
by common signed-policy ID. Only the exact durable policy head can receive a
new event. Updates use immediate redb transactions; counter overflow, missing
records, malformed records and authentication mismatch fail closed.

Counters cover runtime start outcomes, legacy and canonical sync outcomes,
pause/interruption, containment entry and the five zero-tolerance integrity
events. The API accepts only an enum event, not arbitrary labels or metadata.
It therefore has nowhere to record messages, identities, peers, conversations,
addresses, paths or per-message timelines.

Each local record has a keyed BLAKE3 corruption check derived from the local
seed. This detects accidental or out-of-band record edits, but is not remote
attestation: an attacker who controls the complete local database also has the
seed. Likewise, coordinated rollback of the whole database needs an external
or hardware witness to detect. Neither property is overstated as security
evidence.

Health data leaves the store only through
`export_current_health_explicitly(UserOrTestOperatorRequested)`. The JSON
contains the common policy ID, release artifact ID, phase, cohort limit and
aggregate counters. It contains no seed, bucket, timestamp, local path or user
identifier. There is no uploader, endpoint or telemetry service, and absent or
unexported evidence never counts as success.

## Failure and activation boundary

An installation with no policy has no selection and remains legacy-only. A
wrong signing key, bad signature, stale predecessor, epoch gap, phase skip,
phase rewind, oversized cohort or corrupt local evidence is rejected. Exact
policy replay is idempotent.

The crate does not write the Device activation chain, register canonical ALPN,
publish a capability, accept or initiate canonical sessions, or alter runtime
IPC. A later controller must join a verified release decision to the existing
Device-bound ceremony without weakening its restart and containment rules.

## Acceptance and non-claims

Twelve focused tests cover the exact SHA-256 vector, signature tampering,
foreign authority, ordered transitions, containment, seed persistence and
independence, durable counter replay, stale-policy refusal, explicit
privacy-safe export, authenticated corruption rejection and the sixteen-record
bound. The repository boundary has negative mutations and M1 readiness now has
117 gates.

No release build, archive, tag, automatic GitHub build or network execution is
part of M1.1.90. It does not claim public rollout eligibility, anonymous remote
attestation, production release-key distribution, a central telemetry service,
or protection from rollback of the complete local database.
