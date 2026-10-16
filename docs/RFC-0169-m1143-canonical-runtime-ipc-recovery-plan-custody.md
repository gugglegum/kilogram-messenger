# RFC-0169: M1.1.43 canonical history-recovery plan custody

Status: implemented as an inactive M1.1.43 candidate.

## 1. Scope

M1.1.43 specifies deterministic-CBOR requests and Device-signed responses for
approving, inspecting and cancelling one history-recovery plan. It joins the
portable source-signed recovery link from RFC-0138 to a user-confirmed full SAS digest
and an explicit execution policy, then places the resulting
recipient-Device-signed plan under content-addressed local runtime custody.

This slice does not activate canonical CBOR, change Postcard IPC version 26 or
alter the desktop and CLI. It executes no network operation, transfers no
history and installs no OS background service. Recovery execution and its
scheduler remain a later contract.

## 2. Approval and user consent

`approve-history-recovery-plan` carries one independently verifiable signed
recovery link, the exact 32-byte SAS digest confirmed by the local user, the
complete network and external-power policy and a bounded plan lifetime. The
link must name the descriptor Account and the exact descriptor Device as its
recipient. Approval must occur while the link is current, and plan validity is
limited to 3600 through 604800 seconds.

The foreground runtime recomputes the SAS from the signed Account Device list
and both participating Devices. It never accepts a shortened comparison code
as a substitute for the full digest. Only after all context checks pass does
the recipient Device sign the exact link, SAS, execution policy, approval time
and expiry as one canonical history-recovery plan.

Device-link enrollment remains a separate one-shot bootstrap helper. That
ceremony needs Account Root authorization before an ordinary runtime identity
exists, so moving its Root or pre-enrollment authority into routine IPC would
weaken the trust boundary rather than complete it. Its portable authorization
objects are already canonical; its local JSON bootstrap control contract is
explicitly outside this Postcard-to-CBOR migration.

## 3. Custody and status

The local record ID commits the complete signed plan and the optional signed
cancellation tombstone. `history-recovery-plan-status` names a nonzero plan ID
and returns the exact Account- and recipient-Device-bound custody state. An
approved record contains no cancellation. A cancelled record contains exactly
one matching tombstone and preserves the original plan byte-for-byte.

Every successful result reports `restart-not-required`,
`foreground-runtime-only` and `os-background-service-enabled=false`. These are
security-relevant assertions, not UI hints: closing the messenger stops future
execution. The stored policy may permit Ethernet, Wi-Fi, mobile or unknown
networks and may require external power, but custody itself does not open a
connection.

## 4. Cancellation and concurrency

`cancel-history-recovery-plan` exact-compares both the plan ID and current
record ID. A stale caller cannot overwrite a concurrent state transition. The
runtime creates an immutable recipient-Device-signed `user-requested`
cancellation tombstone whose time cannot precede plan approval, then derives a
new content-addressed record ID.

The request ID is the durable idempotency key. An exact replay may return the
already committed transition. Reusing it for different bytes, cancelling an
unknown plan or supplying a stale record ID fails before mutation.

## 5. Privacy boundary and limitations

This IPC surface carries only signed public authorization material and bounded
custody metadata. Account Root secrets, Device private signing or encryption
keys, private recovery capabilities, decrypted history, recovery-page payloads
and local filesystem paths are forbidden. A response makes no claim that a
source Device is online, that any page can be recovered, that history has
converged or that remote copies were deleted.

The transport, source selection, resumable multi-source page execution,
network-change handling and scheduler outcomes are intentionally left for the
next recovery slice. Keeping those effects separate makes user approval and
cancellation auditable without pretending that consent is delivery.

## 6. Verification and next work

Rust round trips and contextual substitution tests, checked-in golden vectors,
an independent Node implementation of deterministic CBOR, BLAKE3 and Ed25519,
and the bounded decoder mutation corpus protect the candidate. The active
runtime implementation, wire ALPN and immutable `M1.0.0` acceptance remain
unchanged.

The next bounded runtime-IPC work is recovery-plan execution and scheduler
results. The active unpaged conversation-list migration and remaining mutation
results still follow before any explicit IPC activation decision.
