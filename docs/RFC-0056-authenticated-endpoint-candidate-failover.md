# RFC-0056: Authenticated multi-device endpoint candidates and failover (M0.9.34)

Status: implemented in M0.9.34.

## 1. Problem

Before this stage a runtime contact represented one peer account in one
conversation, but it named exactly one peer Device ID and one mutable ticket
file. A peer account could already contain several authorized devices for
ratchet fan-out and history recovery, yet the long-lived runtime could deliver
or synchronize only through the single enrolled device. If that device was
offline, another online device of the same account could not take over.

The change must not replace the stable contact identity. Durable queued
messages and ticket-automation policy are already bound to the contact ID
derived from local account, peer account and conversation.

## 2. Persistent model

The original signed contact remains the primary endpoint and keeps its v1
format and stable contact ID. Each additional peer device is represented by a
local-device-signed append-only `SignedRuntimeEndpointCandidate` record under:

```text
STATE_DIR/runtime/endpoint-candidates/<candidate-id>.endpoint-candidate
```

The authenticated content binds:

- local Account and Device IDs;
- the stable runtime contact ID;
- peer Account and Device IDs;
- conversation ID;
- route policy;
- canonical absolute path of the peer ticket file.

The candidate ID is a domain-separated hash of local account, contact ID and
peer Device ID. Therefore one contact cannot silently replace the descriptor
contract for an already enrolled peer device. Ticket bytes may still be
atomically refreshed at the exact enrolled path.

There are at most four endpoints per contact, including the primary endpoint.
The bound prevents an imported local-state record set from turning one send
into unbounded network work.

## 3. Enrollment and IPC

The existing `AddContact` operation is reused. Importing a ticket for the same
peer account and conversation but a different authorized peer Device ID adds
an endpoint candidate instead of colliding with the stable contact record.
Repeating the exact same device/path/policy is idempotent. Reusing a Device ID
with another path or policy fails closed.

Every ticket still has to pass the complete existing checks: ticket signature,
listener account and active device authorization, route-policy binding,
requester-account binding, local device authorization, peer authority pinning
and prekey-directory observation. The desktop result and conversation read
model expose the enrolled endpoint count. The local IPC wire version is 9.

## 4. Candidate resolution

Resolution treats an unreadable, expired or otherwise invalid descriptor as a
failure of that candidate, not as corruption of every other candidate. It
requires at least one usable signed candidate and applies this deterministic
order:

1. highest embedded Account authority revision first;
2. the original primary endpoint first within the same revision;
3. lexicographically smaller peer Device ID first.

All candidates at the maximum observed revision must contain byte-identical
authority snapshots. A same-revision conflict fails closed. The maximum
snapshot is the authority and prekey source for a newly materialized event.

A lower-revision endpoint may still be used for delivery only when its exact
listener certificate remains present in that maximum signed device list. Its
old snapshot is not installed and its old prekey directory is not used to
create ciphertext. A revoked or certificate-replaced device is removed from
the usable set. Automatic synchronization is stricter: it only tries tickets
at the maximum revision because the existing sync command pins the snapshot
embedded in the selected ticket and must never accept rollback.

The normal peer-authority repository remains the durable high-water. If it has
already observed a revision newer than every readable candidate, preparation
fails at the existing anti-rollback pin rather than dialing an old endpoint.

## 5. Delivery and synchronization

A queued plaintext is still opened and materialized exactly once. The newest
usable complete prekey directory produces one immutable signed fan-out event.
The runtime then tries no more than the four authenticated candidates in the
order above and sends the same event on every attempt. A successful
acknowledgement must:

- be authorized by the conversation membership;
- come from the exact Device ID of the endpoint currently being tried;
- name the same conversation and event;
- have the required causal parent.

Only that acknowledgement commits the existing delivered marker. If all
candidates fail, the existing signed bounded retry schedule is advanced once,
not once per endpoint.

Automatic sync uses the same deterministic candidate set and stops after the
first successful current-authority endpoint. Diagnostics expose attempt index,
candidate Device ID and successful failover count without changing the
replicated protocol or ALPN.

## 6. Compatibility

- Existing contact, queued, materialized, delivered and automation records are
  unchanged and remain readable.
- A contact without candidate records behaves as before.
- Connection ticket v10, event wire format and transport ALPN do not change.
- Runtime IPC changes from v8 to v9; runtime and desktop binaries must be
  upgraded together.

## 7. Security properties

This stage adds endpoint availability, not a new identity source. A candidate
can be enrolled only from a ticket already signed by a device authorized by
the expected peer Account Root. Local candidate records are signed by the
local device and covered by the existing vault-primary Runtime repository and
transaction receipts.

Candidate failover does not weaken authority, prekey or publication
high-water. It also does not rematerialize an event, silently add a newly
enrolled recipient to an old event, or accept an acknowledgement from another
device merely because that device belongs to the same account.

## 8. Honest limits and next work

Candidate discovery is still explicit: the user imports one bootstrap ticket
per peer device. There is no account-signed endpoint-set gossip object yet.
The existing publication refresh operation currently refreshes the primary
device channel; additional candidate ticket paths must remain fresh through
their own subsequent publication lifecycle. A missing or expired descriptor
is skipped, but cannot be reconstructed locally.

The next bounded stage should refresh every enrolled candidate channel with
independent observation high-water and per-candidate results, then let the
desktop distinguish enrolled, currently usable and stale endpoints. Cross-
device observation reconciliation and malicious-store availability remain
separate problems.

