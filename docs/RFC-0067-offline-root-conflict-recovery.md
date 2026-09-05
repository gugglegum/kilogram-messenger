# RFC-0067: Offline Root conflict recovery and sibling convergence (M0.9.45)

Status: implemented in M0.9.45.

## 1. Problem

M0.9.44 could safely rotate a quarantined publication channel, but one CLI
process loaded both the online runtime state and the Account Root directory.
The resulting `.pcr` also had to be copied manually to every sibling Device.
That was an acceptable prototype boundary, not the intended operational model.

M0.9.45 separates the ceremony into public request/response artifacts and lets
an already resolved exact-current Device propagate the Root decision through
the existing source-signed, recipient-HPKE endpoint-announcement channel.

## 2. Online request (`.pcrq`)

With the runtime stopped, the affected Device runs
`runtime-publication-conflict-create-request`. The command loads only local
Device/runtime state and a fresh peer-signed rotated ticket. It does not accept
an Account Root path and prints `root_secret_loaded=false`.

The bounded request contains and authenticates:

- exact local Account, requesting Device and Root authority revision;
- the complete retained signed conflict proof and stable evidence ID;
- peer Account/Device and the old/new publication write keys;
- old/new channel epochs and the unchanged route policy;
- the byte-exact replacement ticket; and
- request time and requesting Device signature.

The request ID is content-addressed over the complete signed request. A changed
ticket, proof, epoch or summary therefore produces a different request and
invalidates the Device signature.

## 3. Offline Root response (`.pcrp`)

`account-publication-conflict-authorize` accepts only the offline Root
directory, one `.pcrq` and a new response path. It never opens runtime state and
prints `runtime_state_loaded=false`.

Before signing, it independently verifies:

- the request and embedded conflict-proof signatures;
- exact equality with the Root's current authority revision and published
  Device list;
- that both requester and proof detector remain active messaging Devices;
- the peer Device signature, Root authority and freshness of the embedded
  replacement ticket;
- local audience, peer identity, route policy, new write key and strictly
  increasing channel epoch.

The Root-signed resolution v2 binds the request ID in addition to the M0.9.44
evidence, peer identity, old/new keys, replacement-ticket digest, exact
authority revision and authorization time. The self-contained response carries
that resolution and the exact public replacement ticket. It contains no Root
or Device secret.

## 4. Application and sibling propagation

`runtime-publication-conflict-apply-response` accepts only runtime state and one
`.pcrp`. It verifies the Root signature, current authority, local proof,
immutable binding and embedded ticket before using the existing fail-closed
descriptor-first/transactional-resolution commit. Reapplication is idempotent;
the old `.pcf` remains immutable.

Endpoint-announcement bundle v4 may carry the Root resolution next to its exact
replacement endpoint ticket. A recipient accepts it only when it already has:

- the same exact-current Root roster;
- the same durable old endpoint binding; and
- a retained local conflict proof with the same stable evidence ID.

The recipient atomically replaces its descriptor and appends the same `.pcr`.
It cannot bootstrap trust from the resolution, enroll a new endpoint, or clear
an unrelated quarantine. A crash after descriptor replacement but before the
resolution commit remains fail closed and the same bundle safely completes the
retry. Bundle/ACK and IPC v18 report resolution inventory separately from
conflict-proof inventory.

## 5. Desktop incident workflow

The Windows client exposes a `Security incident · publication conflict` panel
for the selected chat. It lists quarantined channels, creates the `.pcrq`, shows
the exact offline signer command and applies the returned `.pcrp`. Both online
operations require the runtime to be stopped.

The panel deliberately has no Account Root directory field and no Root-signing
button. Offline authorization remains an explicit CLI action on the isolated
Root host. After application, the UI tells the operator to restart the runtime;
normal exact-current own-device exchange then propagates the resolution.

## 6. Bounds and compatibility

- Request and response artifacts are bounded to 9 MiB; the embedded ticket is
  still subject to the existing 8 MiB runtime record limit.
- Root resolution format advances to v2.
- Endpoint-announcement bundle advances to v4 and acknowledgement to v2.
- Runtime IPC advances from v17 to v18.
- Connection ticket v11, event, ratchet, sync and transport ALPN formats do not
  change.
- Short-lived v3 bundles and v1 acknowledgements are intentionally rejected and
  must be regenerated.

## 7. Verification

The three-Device regression now proves that C creates a Device-signed request,
the offline Root rejects a request for another Account, and a tampered request
or response fails verification. C applies the response once; C -> A -> B
encrypted announcements then install the same Root resolution without copying
the Root artifact manually. All three retain their detector-specific `.pcf`,
clear only the matching active quarantine and converge on one effective new
publication key. Reapplying the response remains a no-op.

## 8. Honest boundary

This remains human-authorized incident response, not automatic consensus or
proof that the peer itself signed two conflicting publications. An active
compromised local Device can request a denial-of-service rotation, but cannot
produce the Root response. The offline signer currently presents CLI fields,
not a hardened QR/removable-media appliance with a rich evidence viewer.

Automatic propagation requires an exact-current sibling that already holds the
matching conflict evidence. Devices that never received that proof, are
offline past artifact/ticket freshness, or are on another authority revision
still require a fresh request/response or renewed sibling exchange.
