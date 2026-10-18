# RFC-0205: M1.1.81 canonical sync v2 activation semantics

Status: accepted locally; activation semantics verified, active transport
unchanged.

## Purpose

M1.1.80 proved the dormant canonical transport on hosted and physical Windows
hosts, but did not authorize the ordinary runtime to register or dial it. The
old migration-state names were insufficient for that decision. In particular,
`advertise-cbor-v2` did not say whether an endpoint merely published build
support, registered the ALPN, accepted a new session, or initiated one.

This RFC separates those actions before any production integration. Postcard v6
remains the public default and `/cbor/2` remains inactive in M1.1.81.

## Independent transport rights

Every verified activation state produces six explicit rights:

1. keep the legacy Postcard reader enabled;
2. publish the authenticated canonical capability;
3. register the canonical ALPN;
4. accept a new mutually selected canonical session;
5. initiate a new mutually selected canonical session;
6. resume an exact persisted selected canonical session.

Capability publication requires the canonical ALPN and reader to be ready, but
does not itself accept a new canonical session. A peer still needs the existing
authenticated offer and listener acceptance ceremony. This makes
`advertise-cbor-v2` a safe build-readiness announcement rather than an implicit
transport permit.

`dual-reader-prefer-legacy` may accept a canonical session proposed by an
authenticated peer, but does not initiate one. Two such endpoints therefore
continue to use Postcard. `dual-reader-prefer-canonical` initiates canonical
transport when both sides have mutually authenticated the capability.
`canonical-required` is specified for completeness but is outside the current
dark-launch scope.

## Emergency containment is not rollback

Emergency containment is an overlay, not a migration state. It is valid only
when persisted canonical selection evidence already exists. It stops capability
publication and all new canonical accepts and dials. The canonical ALPN remains
registered solely so an exact persisted session can resume. Durable canonical
evidence is retained.

Containment never turns a selected canonical failure into a Postcard retry. A
parse or network error cannot activate containment automatically. Before any
canonical evidence exists, operators must use the already verified explicit
one-step rollback instead.

This distinction avoids the dangerous interpretation that an incident may
erase selected-session evidence and silently reinterpret the same exchange as
legacy traffic.

## Fail-closed boundaries

- `legacy-only` and `advertise-cbor-v2` reject persisted canonical evidence;
- containment without persisted canonical evidence is rejected;
- capability publication without canonical ALPN registration is invalid;
- a new canonical session still requires mutual authenticated selection;
- resume is authorized only for the exact durable selected session;
- selected canonical decode, network or state failure has no legacy fallback;
- missing, malformed, stale or wrong-Device activation authority must never
  enable canonical transport.

## Deliberate non-activation

M1.1.81 adds a transport-independent permission evaluator and its tests. It
does not persist an activation decision, register an ALPN in the ordinary
runtime, publish a new capability, dial a peer, execute a network test, change
the public default, or create a release artifact.

M1.1.82 will add the durable Device-bound local activation authority. Only a
later package may connect verified permissions to the ordinary Iroh runtime.
