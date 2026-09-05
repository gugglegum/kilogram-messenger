# RFC-0065: Durable publication-conflict quarantine (M0.9.43)

Status: implemented in M0.9.43.

## 1. Problem

M0.9.42 rejected two different publication IDs or ticket digests reported for
one `(channel, publication generation)` pair, but the rejection was ephemeral.
After restart the operator could no longer see why import failed, and the same
channel could still be used for delivery or queried from the opaque store.
Arrival order therefore still influenced availability after a detected
security inconsistency.

## 2. Signed local proof

The first detected mismatch creates one append-only `.pcf` record for that
publication channel. It contains:

- the local Account and detector Device IDs;
- detection time, channel and conflicting generation;
- both complete Device-signed publication observations in canonical
  observation-ID order; and
- the detector Device signature over the complete content.

The proof ID is a domain-separated hash of the signed record. Decode verifies
both observation signatures, equal channel/publisher/generation identity,
different publication ID or ticket digest, canonical ordering and the detector
signature. Restart also requires the proof to belong to the local identity, to
have an exact content-addressed filename and to match one durable enrolled
endpoint publication binding.

This is deliberately a **local conflict proof**, not proof that the peer
publisher signed two publications. The propagated observations are assertions
signed by Account Devices and do not embed the complete peer-signed
publication. A compromised or forked Account Device can therefore create a
conflicting assertion and cause fail-closed denial of service. The UI must not
present local detection as global consensus or as a cryptographic accusation
against the peer.

## 3. Atomic first-conflict handling

Endpoint-announcement import still compares all direct and accepted
observations before descriptor or ordinary runtime-state writes. On the first
conflict it performs a transaction containing only the signed `.pcf` record,
reloads and verifies runtime state, and returns a typed quarantine error with
channel, generation and proof ID. No conflicting acceptance or descriptor is
installed.

Only the first proof for a channel is retained. Replaying another conflicting
bundle returns the existing proof ID and does not create a second record.
Incoming bundles that continue to carry evidence for a quarantined channel are
rejected atomically. A Device that already knows the quarantine omits that
channel's observation from its own later exports.

The network listener sends the existing bounded rejection response but treats
successful proof persistence as handled state, so the long-lived runtime stays
online and publishes an IPC change notification. Unexpected import or vault
errors retain the existing fail-closed runtime shutdown behavior.

## 4. Quarantine enforcement

The quarantined channel is excluded before:

- foreground delivery and automatic sync candidate selection;
- ticket-publication HTTP lookup;
- post-fetch installation if quarantine appeared while the request was in
  flight; and
- automatic ticket-refresh scheduling when every channel for the contact is
  quarantined.

For a contact with independent healthy endpoints, those endpoints remain
usable and refreshable. A fallback candidate must still be at or ahead of the
locally pinned peer-authority revision, and an equal revision must encode the
exact same authority snapshot. Quarantining a newer endpoint therefore cannot
silently reactivate an older descriptor containing revoked devices.

There is intentionally no delete, acknowledge or override command. The opaque
store and desktop GUI cannot clear quarantine. Safe resolution requires an
operator audit of the Account Devices and a future explicit Root-authorized
contact/channel re-enrollment ceremony; merely restarting or receiving a newer
claim does not erase evidence.

## 5. IPC and desktop state

Runtime IPC advances from v15 to v16. Endpoint state now has a distinct
`quarantined` value and exposes conflict generation, proof ID and detection
time. Conversation summaries count usable, stale and quarantined endpoints
separately.

The Windows client renders quarantined endpoint rows in red and displays the
compact proof ID together with the required manual device-audit and
re-enrollment disposition. The status is derived by the runtime from verified
DB-primary state; the GUI receives no signing key and no authority to alter the
proof.

Ticket refresh reports quarantined endpoints separately from ordinary stale or
network failures. A mixed contact can refresh all non-quarantined endpoints;
automation stops retrying only when no refreshable publication channel remains.

## 6. Retention and compatibility

- A channel retains at most one proof, with a global defensive limit of 1,024.
- Conflict proofs are not removed by ticket high-water compaction. A checkpoint
  cannot make a security stop disappear.
- Ticket v10, endpoint-announcement bundle v2, ALPN v8, ratchet and event
  encodings do not change.
- IPC v15 descriptors are intentionally rejected by v16 clients and runtimes;
  restarting both recreates the short-lived signed descriptor.

## 7. Verification

The three-Device A -> B -> C regression now creates a valid signed
same-generation mismatch at C and verifies:

1. the import returns a typed quarantine error;
2. accepted evidence is unchanged and exactly one signed proof is stored;
3. tampering with encoded proof bytes is rejected;
4. reload preserves the proof and explicit endpoint state;
5. candidate selection and pre-network ticket lookup refuse the channel;
6. repeated conflict returns the same proof ID without another record; and
7. later export does not forward an observation for the quarantined channel.

The existing monotonic-evidence compaction portion still succeeds while the
conflict proof remains independently durable.

## 8. Remaining work

M0.9.43 makes one detector safe and observable, but the proof is not yet
propagated to sibling Devices. The next stage should add recipient-verified
conflict-proof propagation and an explicit Root-authorized resolution/channel
rotation ceremony. Privacy-preserving delivery to offline Devices and an
external completeness/rollback witness remain separate problems.
