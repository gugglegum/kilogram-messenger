# RFC-0213: M1.1.89 canonical sync v2 public rollout decision readiness

Status: accepted locally; decision is `hold`.

## Decision

Kilogram has enough evidence to describe a safe public rollout, but not enough
to begin one automatically. The machine-readable decision is therefore
`hold`, `eligible=false`, and `public_default=legacy-only`. This is a successful
decision-readiness result, not a failed canonical transport result.

The verified opt-in path, two-host resilience rehearsal, decoder campaigns,
runtime observability and exact desktop restart ceremony remain accepted. They
prove that selected test Devices can use canonical sync v2 safely. They do not
prove that an anonymous public population can be assigned to cohorts, observed
without a permanent identity, compared with an older production executable and
contained operationally.

This manifest is release-decision evidence only. Runtime does not consume it,
and editing it grants no activation authority.

## Mandatory gates and current hold

Every required gate is conjunctive. There is no weighted score, override or
interpretation of missing data as success. An unknown, missing or stale gate
means `hold`.

Five gates remain open:

1. A privacy-preserving cohort authority must assign an installation without
   deriving a permanent account or Device identifier.
2. Bounded durable local health evidence must distinguish success, failure,
   containment and restart without recording peers, conversations, addresses
   or message material.
3. The exact current production executable must interoperate with N-1 across
   upgrade, restart, direct and relay paths.
4. The real cohort controller and health evidence must pass a containment drill.
5. An independent security review must have no unresolved critical or high
   findings before any public cohort may accept canonical sessions.

Until the first four exist, even the non-accepting advertise canary is blocked.
The fifth gate additionally blocks `dual-reader-prefer-legacy` and every later
phase.

## Cohort privacy

The proposed assignment unit is one local installation, not an account,
Device, contact or conversation. A random secret is generated locally and is
neither exported nor synchronized. The only allowed assignment calculation is
the hash of that secret and a signed release-policy identifier, reduced to
10,000 buckets. The seed and bucket are not peer-visible.

This avoids recreating Telegram's permanent public user ID as rollout
infrastructure. Reinstallation may produce a new cohort assignment. Different
Devices of one account are deliberately independent. Cohort state may change
only through a new explicit release decision and a freshly verified runtime;
it cannot change while the process is running.

## Ordered rollout

Phases are ordered and cannot be skipped:

1. `advertise-canary`: at most 1%, seven days, no canonical accept or initiate;
2. `accept-canary`: at most 1%, fourteen days,
   `dual-reader-prefer-legacy`;
3. `initiate-canary`: at most 5%, fourteen days,
   `dual-reader-prefer-canonical`;
4. `expanded-dual-reader`: at most 25%, twenty-eight days;
5. `broad-dual-reader-default`: at most 100%, only through another explicit
   decision manifest.

Each phase has minimum runtime/session evidence in the decision manifest and
requires a separate human release decision. There is no timer-based or
success-counter-based automatic expansion.

Promotion requires at least 99.5% canonical completion, no more than 0.5
percentage points of failure regression against the legacy baseline, and no
more than 0.25 percentage points of runtime-start regression. Missing
denominators are not treated as zero failures.

History loss or duplication, fallback to legacy after canonical selection,
authority/Device mismatch, authenticated downgrade acceptance, or unrecoverable
state corruption have zero tolerance and stop promotion immediately.

## Containment is not erasure

Rollback first stops new enrollment and disables new canonical accept/initiate.
It does not delete canonical evidence, remove the reader required for an exact
selected-session resume, or retry the same selected work through legacy.

An installation may perform the already verified one-step rollback only before
canonical evidence exists. After selection, emergency containment preserves
the reader and exact resume. Every apply still requires signed release or
explicit local authority, a fresh process and exact post-restart verification.

## Privacy of health evidence

Future health evidence is a bounded local aggregate. Export is explicit and
must not include plaintext, ciphertext, account/Device identifiers, peers,
contacts, conversations, IP addresses, relay URLs, paths or per-message and
per-peer timelines. Evidence that was not exported is absent, not successful.

Designing and implementing that aggregate is the next engineering package; it
is intentionally not smuggled into runtime observability by this decision.

## Non-claims

M1.1.89 changes no peer wire, sync wire, runtime IPC or public default. It does
not implement automatic enrollment, collect rollout telemetry, materialize
canonical-native history, retire legacy sync or complete an independent
security review. No release build, ZIP, tag, automatic GitHub build or network
run is part of this decision-only stage.
