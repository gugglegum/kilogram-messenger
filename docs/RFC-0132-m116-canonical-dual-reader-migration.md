# RFC-0132: Canonical dual-reader migration contract (M1.1.6)

Status: accepted as a design-only, inactive migration contract.

## 1. Purpose

Kilogram currently serializes active interoperable Rust data with Postcard.
The deterministic-CBOR work in M1.1.1--M1.1.5 is an inactive conformance
candidate. This RFC defines how a future migration may happen without treating
parser behavior as protocol negotiation and without silently changing an
existing identifier.

`CANONICAL-MIGRATION.json` is the machine-readable source of truth for the
state machine, compatibility matrix and activation requirements. This RFC and
manifest do not activate canonical CBOR, allocate an ALPN/version or change
runtime behavior.

## 2. Selection is authenticated and explicit

A peer may send deterministic CBOR only when both peers advertised the exact
capability and that advertisement is bound to current authority and session
state. The selected transport/message identifier must independently identify
the CBOR generation. Existing Postcard identifiers must never be reused.

Format sniffing is forbidden. A receiver never tries one decoder and then the
other. If the explicitly selected CBOR decoder rejects a payload, the session
fails closed; it does not retry the same payload as Postcard. Likewise,
unknown versions, capability-binding mismatches and identifier/payload
mismatches fail closed.

## 3. Migration states

The forward sequence is:

1. `legacy-only` -- read and write Postcard;
2. `advertise-cbor-v1` -- still read/write Postcard, but advertise readiness;
3. `dual-reader-prefer-legacy` -- CBOR is allowed only after mutual
   authenticated negotiation;
4. `dual-reader-prefer-cbor` -- mutually capable peers choose CBOR, while
   legacy peers remain on their existing identifiers;
5. `cbor-required` -- legacy reading is retired by an explicit later decision.

Each transition has prerequisites recorded in the manifest. A parse error,
network timeout or peer-controlled input can never trigger rollback. Rollback
is an explicit local/release policy action with a monotonic migration epoch and
a recorded reason. A parse error, network timeout or peer-controlled input can
never trigger rollback.

## 4. Compatibility behavior

Any pairing containing a legacy-only peer continues using Postcard. Two
dual-reader peers also use Postcard unless both present a fresh authenticated
CBOR capability. Only that mutually authenticated pairing may use the new
identifier and deterministic CBOR.

This preserves reachability without creating an unauthenticated downgrade
oracle. It also means the new encoder cannot be deployed as an invisible
replacement beneath the old ALPN or message version.

## 5. Activation boundary

Activation remains blocked until all protocol families have complete canonical
schema coverage, a parallel identifier is assigned, the dual-reader exists,
mixed-version/downgrade tests pass, sustained coverage-guided fuzzing has run,
a rollback drill succeeds, and fresh hosted plus multi-host evidence exists.
Independent cryptographic conformance and the bounded mutation corpus are the
only requirements already marked verified.

The present milestone changes no active wire bytes, network behavior, database
format, release artifact or user-visible feature.
