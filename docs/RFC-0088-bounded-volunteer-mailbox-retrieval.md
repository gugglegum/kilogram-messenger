# RFC-0088: Bounded volunteer mailbox retrieval (M0.9.66)

Status: implemented in the ordinary runtime and externally exercised by
M0.9.67. RFC-0091 adds exact lookup for locator-capable mailbox bindings.

## 1. Outcome

An offline recipient can now retrieve an opaque mailbox replica directly from
verified volunteer providers over `kilogram/m0/blind-mailbox/1`. The ordinary
runtime probes at most three providers per mailbox poll and asks for one item
per provider. It commits a valid decrypted event to the local append-only
history before authorizing deletion at the source store.

The existing HTTPS compatibility fallback remains available. It is no longer
required for reading a replica that the recipient finds through its local
authenticated provider registry, but it is retained until a real cross-network
Alice/Bob/volunteer run proves discovery, offline storage, restart and deletion
end to end.

## 2. Bounded discovery and read

The recipient selects a fresh, transport-distinct sample from unexpired offers
already learned through authenticated peer gossip or explicit import. It
excludes its own endpoint and sends capability-authorized `LIST` requests to at
most three providers. Every response is bound to the exact outer request, then
each stored receipt is verified against the selected store key, mailbox, item
and ciphertext before decryption. A page requests one item per provider, so one
poll cannot become an unbounded download or scan.

For legacy bindings this remains bounded probabilistic discovery, not a global
lookup service. RFC-0091 lets a mailbox owner authenticate an exact bounded
store-key set in its capability update. Such a recipient performs indexed
lookup of those keys and never samples unrelated providers, although it may
still need later gossip to learn a fresh endpoint offer for a committed store.
The legacy sampling path does not guarantee immediate discovery.
The provider request leaks the mailbox pseudonym, timing, size and requester's
network identity to each queried volunteer.

## 3. Commit-before-delete boundary

The existing mailbox client verifies and decrypts the stored envelope without
marking it delivered. The runtime then performs the same application commit as
the HTTPS path. Only after that append-only event/history transaction succeeds
does the separate replication ledger durably bind:

- mailbox and item identifiers;
- exact provider store key and transport identity;
- the provider's signed stored receipt;
- the application commit identifier.

Only this durable record can produce a scoped DELETE authorization. The provider
response must contain a valid signed deletion receipt for the exact store,
mailbox, item and stored-receipt ID. A crash or network loss after the application commit
but before the receipt leaves a bounded pending deletion; a later poll resumes
it without making the message uncommitted.

Copies of the same item at different stores are tracked independently. Deleting
one cannot erase the evidence needed to delete another. Expired commit and
deletion records are bounded by the original store receipt lifetimes.

## 4. Compatibility and limitations

This slice adds no executable, listener, scheduled task, Account directory or
central mailbox requirement. Provider LIST and DELETE reuse the same dedicated
Iroh ALPN and capability model as PUT. HTTPS compatibility fallback remains
enabled, and reverse acknowledgements still use the existing compatible upload
path while their own replicated-outbox lifecycle is designed. A reverse
mailbox binding is optional: once the inbound event and acknowledgement are
durably in local history, inability to prepare the reverse upload is logged but
cannot roll back the application commit, suppress the replica-ledger record or
prevent DELETE. Later authenticated history sync can still carry the retained
acknowledgement.

Transport identity diversity is not operator independence or Sybil resistance.
New locator-capable bindings retain the exact recipient-selected replica-set
commitment on both endpoints; old bindings and activations made before two
offers are known retain the visible random fallback. Provider churn can still
delay retrieval until a fresh signed offer for a committed store arrives.
Private information retrieval, padding, push wakeup, erasure coding, reputation
and proof of deletion remain deferred.

## 5. Field-evidence plan

The next field run must keep Bob offline while Alice creates an opaque message,
obtain the configured two-of-three volunteer receipts, stop Alice, then start
Bob with the authenticated provider offers. Bob must retrieve through Iroh,
durably commit the event, delete every discovered copy with signed receipts and
produce the same local history after restart. For a mechanism-only test,
multiple transport-distinct provider processes may share one operator machine;
that does not count as operator-independent durability evidence.

## 6. Verification

Network-free tests cover application-commit-before-delete, exact source binding,
signed receipt verification, replay, restart state and expiry cleanup. The
static `scripts/verify-kilogram-volunteer-retrieval-boundary.ps1` gate checks
the three-provider/one-item bounds, Iroh LIST/DELETE carrier, durable source
records, deletion ordering and receipt, HTTPS compatibility marker and the
no-new-EXE boundary. It deliberately does not claim external delivery evidence.
