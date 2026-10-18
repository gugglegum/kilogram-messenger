# RFC-0207: M1.1.83 production runtime canonical sync v2 dark launch

Status: accepted locally; production runtime integration verified under
explicit per-Device opt-in, public default unchanged.

## Purpose

M1.1.83 connects the previously verified canonical sync v2 state machine,
typed store and Iroh adapter to the ordinary `kilogram-cli` runtime. This is a
dark launch, not a public migration. A new or existing state directory without
the Device-signed activation chain still registers and dials only Postcard v6.

The canonical ALPN is registered only when the verified M1.1.81 permissions
allow it. A ticket publishes the canonical candidate only after that reader is
registered. A requester dials `/cbor/2` only after both Devices complete and
persist the authenticated selection ceremony over an already authenticated
legacy connection.

## Default and opt-in endpoint construction

Runtime startup loads the M1.1.82 activation authority before binding Iroh.
Missing authority produces the implicit `legacy-only` policy. Malformed,
forked, stale, wrong-Device or high-water-mismatched authority fails closed;
there is no environment variable or convenience startup flag that grants
canonical rights.

The endpoint ALPN list is derived from the six verified permissions:

- the ordinary default contains only `kilogram/m0/sync/8`;
- an explicitly enabled reader also contains `kilogram/m1/sync/cbor/2`;
- capability publication is derived from the same policy and therefore cannot
  precede reader registration;
- the default signed ticket carries no canonical candidate;
- ticket refresh and directory rotation preserve the current verified policy
  instead of silently adding or dropping the candidate.

## Authenticated selection and durable evidence

Candidate negotiation is a bounded extension of the authenticated Postcard
control protocol. The requester Device signs an exact offer; the listener
verifies both Account authorities, both Device certificates, membership,
ticket capability, conversation, stable role-bound session binding and a
selection epoch derived from the listener ticket publication epoch. The
listener signs an acceptance bound to that exact offer.

Both sides persist a content-addressed selection record containing the ticket
capability, offer, acceptance, both certificates and both authority snapshots.
The listener persists before returning acceptance; the requester persists
before the canonical dial. A restart can therefore authorize only the same
conversation, roles and selected ALPN. A selected session cannot be inferred
from a state name or a peer claim.

Activation epochs belong to different local Device chains and are not compared
across peers. The cross-peer selection epoch instead uses the signed listener
ticket publication channel epoch plus one, which both sides can verify without
pretending their independent local administrative histories share a clock.

## Shared canonical runtime

The production path calls the same shared Iroh adapter used by the bounded
network evidence lane. Legacy history is imported through the authenticated,
bounded and resumable bridge into the typed carrier store. The client and
responder then restore or bootstrap the signed checkpoint and checkpoint-bound
frame journal and run over the exact canonical ALPN.

On completion, only closed `LegacyPostcardV6` carriers are materialized back
into the active local history during this dark launch. Canonical-native event
materialization is intentionally not enabled. Any decode, authorization,
network, checkpoint or state error after selection ends that attempt. It never
turns a selected canonical failure into a Postcard retry.

## Stable privacy-bounded telemetry

The CLI reports policy state, migration epoch, containment, reader and
publication permissions, selected ALPN, selection evidence ID and explicit
`sync_transport_fallback_after_selection=false`. It does not log message
contents, private keys or activation reasons as transport telemetry.

## Local control result and remaining work

One debug binary completed three fresh-state direct scenarios:

1. default/default selected Postcard v6 and converged;
2. opt-in/default selected Postcard v6 and converged;
3. opt-in/opt-in selected `/cbor/2`, persisted one matching evidence ID on
   both sides and converged to identical six-event histories.

The first production integration deliberately accepts one newly selected
canonical session per listener runtime process. Restart/resume, deliberate
selected-path failure, rollback, containment and a two-host run remain M1.1.84
work. Desktop UI rollout, public default activation, canonical-native history
and legacy retirement are outside this RFC.
