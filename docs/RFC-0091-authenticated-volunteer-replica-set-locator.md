# RFC-0091: Authenticated volunteer replica-set locator (M0.9.68)

Status: implemented for newly activated or rotated mailbox capabilities when
the owner already knows at least two active volunteer providers.

## 1. Outcome

The recipient no longer has to discover an offline message by repeatedly
sampling unrelated providers from a potentially large local registry. Before
the recipient may go offline, it selects a bounded set of volunteer store keys
and places that exact set in the existing Device-signed mailbox capability
update sent to the one authorized writer.

The update already travels inside an authenticated encrypted Device session.
It is retained by both endpoints and is not published as a social directory.
The commitment contains only canonical volunteer `store_key` values. It does
not contain an Account ID, Device ID, conversation ID, mailbox ID, capability,
endpoint or message item ID.

## 2. Commitment contract

`MailboxReplicaSetCommitment` contains two to eight distinct store keys in
canonical order. The current runtime selects at most three transport-distinct
providers. The entire capability update, including the mailbox binding and
replica set, is signed by the mailbox owner Device. A domain-separated
commitment ID binds the canonical set to that mailbox binding.

The old `Activate` action remains byte-compatible. A new
`ActivateWithReplicaSet` action was appended to the action enum, so retained
v1 activations, rotations and revocations continue to decode and verify. An
old capability has no implied locator and remains in the explicitly reported
legacy probabilistic mode.

The commitment proves what the mailbox owner selected. It does not prove that
the volunteers are independent operators, retain data, have the advertised
capacity or remain online. Store-signed PUT receipts remain the durability
evidence.

## 3. Sender path

At mailbox activation or rotation the ordinary runtime reads only verified,
unexpired offers from its bounded local registry. If at least the current
two-receipt threshold is available, it chooses up to three transport-distinct
stores, canonicalizes their keys and signs them into the capability update.

When a message falls back to mailbox delivery, the sender copies that exact
commitment into a separate immediate-durability Redb locator record beside the
existing per-item replication plan. Restart cannot silently replace the set:
an unequal replay fails closed. The runtime resolves the committed keys by
indexed registry lookup and may use a refreshed signed endpoint offer for the
same store key. It does not substitute a newly sampled store. Missing offers
leave replication incomplete until gossip supplies a current offer or the
recipient rotates the mailbox capability.

Idle discovery and retry inspection open the provider and replication Redb
files read-only. This matters because Redb may change internal file bytes on a
writable open even when no logical record changes, while Kilogram's encrypted
state vault authenticates the exact bytes. A write or expiry cleanup therefore
remains an explicit mutation that must be mirrored; an idle poll cannot create
or silently rewrite a database.

The locator record stores mailbox/item pseudonyms already present in the
replication ledger, the opaque dispatch binding, commitment ID and store keys.
It stores no Account, Device or conversation identifier. Cleanup removes the
locator atomically with the expired replication plan.

## 4. Recipient path

The recipient retains the same signed activation in its local capability
chain. For each active receive binding it obtains the exact committed store
keys and performs indexed lookups in the provider registry. It probes no more
than the three committed providers and asks for one opaque item per provider,
then keeps the RFC-0088 application-commit-before-signed-DELETE ordering.

The signed store offer remains necessary because a store key alone is not a
network endpoint. Provider gossip therefore continues to distribute short-
lived signed endpoint offers, but it no longer decides which unrelated stores
the recipient should scan for this mailbox.

## 5. Compatibility and privacy boundary

If activation occurs before two provider offers are known, the runtime creates
the unchanged legacy capability and visibly reports
`legacy-random-fallback`. A later explicit mailbox rotation can install an
authenticated exact locator. Automatic route-only rotation is deferred so it
cannot race the existing ordered capability lifecycle.

Providers see only requests addressed to their own store, including the
existing pseudonymous mailbox ID, timing, size and requester IP/relay path.
They do not receive the full replica set or the capability update. Colluding
selected providers can still correlate identical ciphertext and mailbox
pseudonyms. This RFC does not add PIR, padding, Sybil resistance, proof of
capacity, proof of deletion, a global directory, a server or a new executable.

HTTPS mailbox upload remains a compatibility copy for this milestone. Its
removal requires a clean external run created under one locator-capable
revision, plus a decision for capabilities provisioned before enough provider
offers were available.

## 6. Verification

Network-free tests cover canonical set construction, Device-signature and
round-trip verification, legacy action compatibility, conflicting durable
locator replay, restart retention, expiry cleanup and indexed exact provider
lookup. `scripts/verify-kilogram-volunteer-replica-locator-boundary.ps1`
fails closed if the signed action, bounded set, durable locator table, exact
sender/recipient lookup, legacy marker, privacy exclusions or no-new-EXE
boundary disappears.
