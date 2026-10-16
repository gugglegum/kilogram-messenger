# RFC-0185: M1.1.59 dormant cross-format membership policy

Status: accepted locally; concrete policy verified, active runtime unchanged.

## Outcome

`kilogram-session` now contains `CanonicalSyncV2MembershipPolicy`, the concrete
pre-storage authorization policy for closed `/cbor/2` event carriers. It can be
constructed from either an installed Postcard `ConversationMembershipSnapshot`
or a future canonical `CanonicalSignedConversationMembershipV1`. The selected
snapshot signature is verified at construction and again while an event is
authorized.

One owner-signed snapshot format authorizes both carrier namespaces. The full
matrix is intentional:

| Membership snapshot | Legacy Postcard carrier | Native canonical carrier |
| --- | --- | --- |
| installed Postcard | verified | verified |
| canonical CBOR | verified | verified |

This prevents event encoding from silently changing the membership semantics.
It also means an existing conversation owner is not required to re-sign old
membership state under a new signature domain merely so a native canonical
event can be checked during the transition.

## Fail-closed checks

Every call exact-binds the requested conversation, snapshot conversation and
carrier conversation. It repeats the carrier's own authorization verification,
then requires the carrier author Account in the signed member set. A nonmember,
wrong conversation, malformed signature or invalid carrier is rejected before
the state machine invokes the atomic typed store.

The policy selects its decoder when it is constructed. It does not sniff
membership bytes and never changes snapshot format after a parse or policy
error. Snapshot revision remains visible for later checkpoint/import binding;
this stage does not invent a local latest-revision rule.

## Non-activation

The policy is wired only to the dormant transport-independent adapter contract.
The active Postcard runtime still loads and enforces its existing membership
path. `/cbor/2` remains unregistered and undialed. There is no legacy history
import, typed checkpoint transaction or mutual capability-driven dial path.

## Remaining activation work

Activation remains blocked on:

1. bounded, resumable import of verified installed legacy history;
2. transactional coupling of import/store state to the typed checkpoint;
3. mutual authenticated capability selection with no error fallback;
4. mixed-version, downgrade and rollback regression;
5. sustained fuzzing and fresh hosted plus physical-device evidence.
