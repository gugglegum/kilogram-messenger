# RFC-0064: Convergent sibling publication evidence (M0.9.42)

Status: implemented in M0.9.42.

## 1. Problem

M0.9.37 preserves a source Device's signed ticket-publication observation in a
recipient-Device-signed `.aeo` acceptance record. M0.9.38-M0.9.41 transport the
announcement automatically, but an accepted observation stopped at that first
recipient: only observations made directly by the next bundle source were
exported. A three-Device Account could therefore retain different freshness
high-waters indefinitely even while its Devices exchanged endpoint bundles.

The acceptance set also shared a global record limit but had no checkpointed
per-channel compaction. Periodic automatic announcements could retain repeated
or obsolete evidence until the limit was reached.

## 2. Bundle v2 observation choice

For each enrolled endpoint channel, endpoint-announcement bundle v2 carries at
most one observation claim:

- the source Device's latest direct signed observation; or
- the source Device's signed acceptance of another Device's observation.

The source selects the higher publication generation. Equal generations must
have the same publication ID and ticket digest; otherwise export fails closed.
The outer bundle still carries the exact current Root-signed Account device
list, is signed by its source and HPKE-encrypted to one exact recipient.

Forwarded evidence preserves both signatures: the original observer signed the
publication observation, and the forwarding Device signed its acceptance.
Bundle verification additionally requires the evidence witness to be the
bundle source. A recipient never treats an arbitrary copied `.aeo` file as
local state.

## 3. Recipient convergence

The unchanged import transaction unwraps either form to the original signed
observation and compares it with every direct and accepted local observation
for the same `(channel, publication generation)` pair. A different publication
ID or ticket digest is authenticated same-generation divergence and rejects
the complete bundle before descriptor or vault writes.

When the claim is compatible, the recipient creates its own Device-signed
acceptance record around the original observation. That local receipt can be
selected for a later bundle, so evidence advances across A -> B -> C without a
shared folder, a trusted store or a direct A -> C session. An already accepted
publication tuple is not written again, even if it arrives through another
bundle or witness.

This is monotonic high-water convergence, not consensus. A conflict is detected
only after contradictory signed claims reach the same Device. The opaque store
is an untrusted carrier and never attests completeness, global order or the
absence of another publication.

## 4. Bounded retention

Accepted evidence now participates in the existing runtime-ticket compaction
trigger. After more than eight retained records for a channel, the runtime:

1. verifies the DB-primary bytes selected for removal;
2. retains one signed highest-generation `.aeo` record;
3. adds an `AcceptedEndpointObservation` anchor containing its exact channel,
   generation, publication ID, ticket digest and evidence ID;
4. commits the new Device-signed cumulative checkpoint and removals in one
   state transaction.

Restart requires the retained evidence to match the checkpoint anchor. Later
compaction may advance the generation but cannot replace an equal-generation
anchor with a different record or drop the authenticated channel.

## 5. Compatibility and security boundary

- Endpoint-announcement bundle encoding and signing domains advance to v2.
  Old v1 announcement files are intentionally rejected; they are short-lived
  diagnostic/transport artifacts and can be regenerated.
- Runtime IPC remains v15 and its `observation_count` continues to mean the
  number of endpoint observation claims in the bundle. Ticket v10, ALPN v8,
  pairwise locator, ratchet and event formats do not change.
- Existing v1 `.aeo` records remain valid. The checkpoint enum adds its new
  variant at the end, preserving the discriminants of existing anchors.
- Revoking an original observer does not erase historical accepted high-water.
  New bundles still require their current forwarding source and recipient to
  be active in the exact current Root roster.
- A compromised active Device can create a contradictory signed claim and
  cause a visible fail-closed denial of import. No serverless protocol can both
  ignore such authenticated equivocation and claim that all authorized Devices
  share one history; recovery and device-removal UX remain separate work.

## 6. Verification

The three-Device regression proves direct observation A -> accepted evidence B
-> accepted evidence C, with the same publication high-water at C. It then
injects a validly signed conflicting same-generation observation from B and
verifies that C rejects it without adding evidence. The same scenario creates
nine monotonic accepted generations, compacts them to one high-water and
successfully reloads the matching checkpoint anchor.

## 7. Remaining work

Devices that never exchange an announcement still cannot discover each other's
conflict. Privacy-preserving gossip/mailbox delivery, a durable conflict-proof
UX, external rollback witnesses, cross-machine scheduling leases and optional
OS background execution remain separate stages.
