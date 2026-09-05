# RFC-0057: Multi-candidate ticket refresh and endpoint health (M0.9.35)

Status: implemented in M0.9.35.

## 1. Problem

M0.9.34 lets one stable contact contain up to four authenticated peer Device
endpoints and fail over delivery or synchronization between them. Ticket
publication refresh still addressed only the original primary endpoint. An
alternate endpoint could therefore expire even while the foreground automation
reported success for the contact, and the desktop showed only an enrolled
candidate count rather than which candidates were currently usable.

One refresh action must cover the complete bounded enrollment snapshot without
making one failed publication service lookup erase successful work for another
Device.

## 2. Bounded execution model

The runtime snapshots the stable contact and all of its signed endpoint
candidates under the state lock. The existing limit of four candidates is also
the hard limit for one refresh action.

For every candidate the runtime independently:

1. reloads and authenticates the exact enrolled descriptor contract;
2. derives the candidate-specific opaque publication channel from that ticket;
3. fetches the encrypted publication from that channel;
4. rechecks the unchanged enrollment, membership, requester authorization,
   publisher Account/Device, route policy and channel after the fetch;
5. enforces the channel's own signed monotonic observation high-water;
6. pins the published authority, observes the complete prekey directory and
   atomically replaces only that candidate's descriptor file.

Network GETs for valid prepared channels run concurrently so four slow channels
do not multiply the local IPC deadline. All trust, ratchet, observation and
descriptor commits run sequentially. This avoids competing state transactions
while retaining independent network availability.

## 3. Partial results and automation

The IPC response contains one typed result per enrolled peer Device. A
successful result is `usable` and includes its channel, publication generation,
expiry, authority revision and local observation outcome. A failed result is
`stale`, preserves the candidate identity and descriptor contract, and carries
a bounded local diagnostic. A known channel is retained in the failed result
when preparation succeeded.

Successful candidates commit immediately even when another candidate fails.
The aggregate response has exact total/refreshed counts and `complete=false` for
partial success. Manual IPC therefore reports useful partial progress rather
than converting the whole operation into an opaque error.

Foreground ticket automation records success only when every candidate in the
snapshot refreshed. Partial completion records one normal failed attempt and
uses the existing exponential backoff; already-observed successful channels are
idempotent on retry. On complete success the earliest candidate expiry schedules
the next refresh. The minimum non-zero publication generation is retained only
as conservative aggregate attempt diagnostics because generations from
different channels are not a shared sequence.

## 4. Endpoint health read model

Runtime IPC v10 extends each conversation summary with the bounded candidate
list and exact `usable`/`stale` counts. Each item exposes:

- primary/alternate role and peer Device ID;
- route policy and pinned descriptor path;
- current authority revision and opaque publication channel when readable;
- the local observation publication high-water and observation time;
- a typed `usable` or `stale` state plus a diagnostic reason.

`usable` means the descriptor is currently valid, belongs to the newest
byte-consistent readable authority revision, and its exact certificate is
active in that revision without falling behind the durable pinned peer-authority
high-water. Missing, expired or invalid descriptors, older authority revisions,
authority rollback, revoked/replaced certificates and same-revision authority
equivocation are shown as `stale`. The Windows desktop displays the aggregate
counts, every candidate state and its local publication high-water.

## 5. Security and compatibility

- The stable contact ID and all M0.9.34 candidate records remain unchanged.
- Publication observation chains remain independent per opaque channel; no
  aggregate counter can roll one channel back or substitute another Device.
- Partial network failure does not weaken signature, expiry, membership,
  authority or canonical-path checks.
- Event, ratchet, sync, ticket v10 and transport ALPN formats do not change.
- Local IPC changes from v9 to v10, so runtime and desktop executables must be
  upgraded together.

## 6. Honest limits and next work

This stage refreshes only candidates that were explicitly enrolled already. It
does not discover another device automatically, reconcile observation
high-water between the user's own devices, or prove availability against a
malicious publication store.

An expired descriptor cannot currently reveal its publication channel through
the strict ticket decoder. Automation is intended to refresh before expiry, but
a device returning after that window still needs a fresh bootstrap ticket. A
future signed, expiry-independent channel binding should remove that recovery
dead end without accepting an expired transport endpoint.
