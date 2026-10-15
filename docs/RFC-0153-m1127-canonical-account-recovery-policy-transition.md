# RFC-0153: Canonical Account recovery policy transition

Status: implemented as an inactive M1.1.27 candidate.

## 1. Scope

This stage completes the portable Account recovery surface with a deterministic
CBOR proof for changing the recovery electorate. It adds a transition request,
Device-signed approvals from the old and new rosters, and a joint-quorum
certificate. It does not activate a new reader or writer and does not change
the active Postcard v1 recovery artifacts.

The portable certificate deliberately does not contain the local recovery
policy database. A caller must compare the certificate's old epoch, previous
transition ID and old roster digest with a locally authenticated monotonic anchor before
installing it.

## 2. Monotonic successor

The request embeds independently verifiable old and new Root-signed recovery
packages and their matching Root witnesses. Both packages must name the same
Account, and the new authority revision must be strictly greater.

The successor retains every old revocation verbatim. It retains every old
conversation membership at the same or a higher revision without removing an
old member. Every old Device certificate remains byte-identical unless that
Device is explicitly revoked in the new authority snapshot. Finally, the new
recovery roster digest must differ from the old one, so the transition cannot
be used as a no-op epoch increment.

Epoch zero requires a zero previous transition ID. Every later epoch requires
a non-zero predecessor, which makes the portable certificates linkable into a
single history after the caller checks the local anchor.

## 3. Joint old/new consent

Each approval binds the exact Request ID, Account ID, old and new epochs,
previous transition ID, both recovery-roster digests, challenge, request
expiry, approver Device ID, the approver's preceding transition-approval head,
and approval time. An approver must be active in at least one of the two
certified rosters.

Approvals are strictly ordered and distinct by Device ID. A certificate is
valid only with a strict majority of the old roster and a strict majority of the new roster.
A Device present in both rosters contributes once to each
majority; duplicate approval bytes never add votes. This joint rule prevents a
new, self-selected electorate from replacing the old recovery policy by
itself, while still requiring the proposed new electorate to acknowledge the
state it is inheriting.

## 4. Freshness, bounds and identity

The request carries a non-zero 256-bit challenge and a positive lifetime of at
most 1,800 seconds. Request, approval and certificate identities are separate
domain-separated BLAKE3 digests over their complete deterministic-CBOR bytes.
Approval signatures use a distinct Ed25519 domain.

The request is bounded at 67,182,592 bytes, each approval at 16 KiB, the
approval list at 64 entries and the certificate at 68,296,704 bytes. Strict
decoders reject trailing bytes, indefinite or non-minimal encodings, invalid
nested signatures, substitutions, rollback and insufficient joint quorum.

## 5. Activation

This candidate makes canonical coverage of the existing Account recovery
portable artifacts complete, but that is not activation. Runtime IPC and
transport negotiation remain separate migration surfaces. Activation still
requires an explicit version migration, dual-reader and downgrade tests,
sustained coverage-guided fuzzing, and fresh multi-device field evidence.
