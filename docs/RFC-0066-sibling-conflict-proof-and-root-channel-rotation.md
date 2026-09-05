# RFC-0066: Sibling conflict proof and Root-authorized channel rotation (M0.9.44)

Status: implemented in M0.9.44.

## 1. Problem

M0.9.43 made a contradictory same-generation publication claim durable on the
Device that observed it, but another Account Device could continue using the
same channel. Quarantine also had no safe exit: deleting `.pcf` would erase the
audit trail and accepting another ticket on the same pinned channel would only
hide the inconsistency.

M0.9.44 therefore separates three authorities:

1. an Account Device may report and relay what it observed;
2. the peer Device must issue a fresh signed descriptor on a genuinely new
   publication channel; and
3. the local Account Root must explicitly authorize replacing one exact
   quarantined channel after operator audit.

No store, GUI, sibling Device or newer unaudited claim can perform step 3.

## 2. Conflict-proof propagation

Endpoint-announcement bundle v3 carries at most one of the following per
endpoint:

- a direct/accepted monotonic publication observation; or
- the complete `SignedPublicationConflictProof` retained for the channel.

The bundle remains source-Device signed, recipient-HPKE encrypted and bound to
the byte-exact current Root-signed Device list. A forwarded proof is accepted
only when:

- both original observation signatures and the detector signature verify;
- the detector is an active Device in that exact list;
- local Account, peer Account, peer Device, channel and durable endpoint
  binding all agree; and
- the endpoint carries no competing ordinary observation.

The recipient does not store another Device's `.pcf` as local truth. It signs
the same canonical observation pair with its own Device key and persists one
local `.pcf`. This makes restart validation and ownership unchanged from
M0.9.43 while allowing A -> B -> C propagation.

`proof_id` remains detector/time specific. A second `evidence_id` hashes only
the canonical pair of complete signed observations, so all honest recipients
derive the same identifier. This stable evidence ID is what the Root resolution
binds.

Import is idempotent. A Device already quarantined on that channel retains its
first proof. Bundle and network acknowledgement counts distinguish ordinary
observation evidence from conflict evidence. Runtime IPC advances to v17.

## 3. Real channel rotation

The old publication capability was deterministic for
`(Device secret, peer Account)`. Reissuing a ticket could therefore not resolve
a channel conflict.

Ticket v11 adds `ticket_publication_channel_epoch` under the existing listener
Device signature. Epoch zero preserves the original v10 capability derivation.
Every non-zero epoch uses a separate domain-separated derivation and therefore
produces a different Ed25519 write key and opaque channel ID.

`runtime-publication-channel-rotate` appends a local-Device-signed `.pcrn`
record for one enrolled peer Account. Epochs must start at 1, remain contiguous
and have non-regressing timestamps. The next runtime start reads the highest
verified epoch and publishes its ticket with that capability. Rotation is
deliberately explicit and restart-required in this slice; it is not silently
triggered by a remote report.

## 4. Root resolution artifact

After receiving the peer's fresh ticket, the local operator runs
`runtime-publication-conflict-authorize-resolution`. The command requires the
quarantined runtime state and the offline Account Root. It verifies:

- the Root, runtime certificate, current authority snapshot and published
  Device list are byte-exact current state;
- the selected channel has a retained local conflict proof and durable endpoint
  binding;
- the replacement ticket is fresh, peer-Device signed, targets the local
  Account and preserves peer Account/Device and route policy;
- its channel epoch is strictly higher and its write key/channel is different;
  and
- the new channel is not itself quarantined.

The no-clobber Root-signed resolution binds the exact current authority
revision, stable conflict evidence ID, peer identity, old and new write keys,
the exact replacement-ticket digest and authorization time.

`runtime-publication-conflict-apply-resolution` verifies the artifact and the
same local evidence, atomically replaces the external descriptor, then appends
the `.pcr` resolution record through the normal transactional runtime-state
path. Replacing the descriptor first is fail closed: a crash before the `.pcr`
commit leaves a new descriptor that still disagrees with the old binding and is
unusable; retrying the same command completes safely.

The original binding and `.pcf` are never deleted. The verified `.pcr` acts as
an explicit Root-authorized override from the old pinned key to the exact new
key. Delivery, sync, refresh, automation status and endpoint announcements use
only that effective key. Reapplying the same resolution is idempotent; a second
different resolution for the same old channel is rejected.

Because the artifact uses stable evidence ID rather than detector-specific
proof ID, the same resolution can be applied on every exact-current sibling
that retained the same conflict evidence.

## 5. Operator sequence

On the peer Device whose publication channel must change:

```powershell
.\kilogram-cli.exe runtime-publication-channel-rotate `
  --state-dir .\state-peer `
  --peer-account <LOCAL_ACCOUNT_ID>
```

Restart that runtime and transfer its newly published ticket to the local
operator. On one audited local Device with access to Account Root:

```powershell
.\kilogram-cli.exe runtime-publication-conflict-authorize-resolution `
  --state-dir .\state-local `
  --account-dir .\account-root `
  --channel <QUARANTINED_CHANNEL_ID> `
  --replacement-ticket-file .\peer-rotated.ticket `
  --output-file .\channel-resolution.pcr
```

Apply the same resolution and exact replacement ticket to every current local
Device that has the matching propagated evidence:

```powershell
.\kilogram-cli.exe runtime-publication-conflict-apply-resolution `
  --state-dir .\state-local `
  --resolution-file .\channel-resolution.pcr `
  --replacement-ticket-file .\peer-rotated.ticket
```

The runtime must be stopped while these state commands hold its exclusive
state lock.

## 6. Bounds and compatibility

- At most 1,024 rotation records and 1,024 Root resolutions are loaded.
- Conflict proofs retain the existing 1,024-channel bound.
- Ticket v10 and endpoint-announcement bundle v2 are intentionally rejected;
  both are short-lived artifacts and must be regenerated.
- IPC v16 descriptors are intentionally rejected by v17 peers; restart runtime
  and desktop together.
- Event, ratchet, sync and transport ALPN encodings do not change.

## 7. Verification

The three-Device regression now proves:

1. C detects a valid signed mismatch and persists one local proof;
2. C -> A -> B propagation creates local proofs with different proof IDs but
   one stable evidence ID;
3. replay adds no record, and a tampered proof is rejected;
4. a tampered Root artifact or wrong replacement ticket is rejected;
5. one Root resolution applies to all three Devices and retains every `.pcf`;
6. the old channel stops being active while the rotated descriptor becomes a
   usable candidate; and
7. post-resolution endpoint-announcement exchange uses the new channel without
   resurrecting the old quarantine.

The capability unit test also proves epoch-zero compatibility and separation
between the original and rotated write keys.

## 8. Honest boundary

This is a manual incident-response ceremony, not automatic consensus. A
compromised active Account Device can still create a false local conflict and
cause denial of service; Root authorization means the human accepted a
specific recovery after audit, not that peer equivocation was cryptographically
proven.

Resolution artifacts are not yet distributed automatically in sibling
bundles, and peer-side rotation requires a runtime restart. The CLI command
that issues a resolution intentionally needs both runtime evidence and the
offline Root directory in one local process; a future production UX should
split that into a QR/removable-media request and offline signer response.
