# RFC-0085: Bounded volunteer provider registry and selection (M0.9.63)

Status: implemented as a verified local import and selection boundary.
Automatic network gossip and mailbox replication are not included.

## 1. Outcome

Kilogram clients can now retain a small set of fresh, store-signed volunteer
storage offers and deterministically choose several transport-distinct
providers. This is the first client side of provider discovery; it turns the
manual offer emitted by M0.9.62 into durable, usable selection state.

The registry is deliberately not a contact book. A record contains only:

- the exact signed offer bytes;
- the offer's unrelated mailbox store public key and lifetime;
- a local observation time;
- an opaque transport-identity digest derived from the Iroh endpoint ID.

It contains no Account ID, Device ID, conversation ID, mailbox ID, read
capability or write capability. The mailbox capability remains selected by the
delivery record only after a provider has been chosen.

## 2. Verified bounded import

The authenticated local runtime actor accepts one base64url offer through
`ImportVolunteerStorageOffer`. Import fails closed unless:

1. the decoded object is within the protocol size limit;
2. the store-key signature is valid at the current time;
3. the encoding is canonical;
4. the enclosed endpoint is a valid Iroh endpoint descriptor;
5. a replacement for the same store key has a strictly later signed issue
   time.

An exact replay is idempotent and does not refresh its local observation time.
A different object with the same signed issue time is rejected instead of
silently selecting one fork.

The Redb registry defaults to 256 store keys and has a hard configurable bound
of 4096. Expired, cryptographically verified records are removed during an
import before capacity is evaluated. A full live registry rejects new offers;
it does not permit attacker-controlled silent eviction of an existing choice.

The initial carrier is still explicit/manual. This slice does not claim a
global directory or anonymous discovery network. A later bounded gossip layer
may carry the same signed bytes without changing their trust model.

## 3. Deterministic independent selection

`SelectVolunteerStorageProviders` accepts an opaque 32-byte selection salt and
a count from one through eight. The runtime ranks all fresh offers using a
domain-separated BLAKE3 score over:

- the caller-provided salt;
- store key;
- parsed transport identity;
- signed-offer ID.

Sorting that score gives the same result after restart and on replay. At most
one offer per parsed Iroh endpoint identity is returned, even if that endpoint
advertises several store keys. This is a useful provider-diversity floor, not
Sybil resistance: one operator can still create many endpoint identities.

The salt must be freshly random per replicated item. Account, Device,
conversation, mailbox capability or stable user identifiers must not be used
as the salt because that would make provider choices linkable. The diagnostic
CLI generates a random salt when none is supplied and prints it so a selection
can be reproduced.

Selection is local policy, not proof that a provider is online, honest, has the
advertised capacity, or will retain an item. Store-signed PUT receipts remain
the only durable-acceptance evidence.

## 4. Runtime and operator surface

The existing authenticated loopback IPC gained two commands and corresponding
secret-free responses. The diagnostic CLI exposes them as:

- `runtime-ipc-volunteer-provider-import --ipc-file ... --offer-file ...`;
- `runtime-ipc-volunteer-provider-select --ipc-file ... [--selection-salt ...]
  [--count 3]`.

Only public offer metadata is returned: offer ID, transport digest, store key,
policy class, capacity hints and times. Exact endpoint bytes remain inside the
runtime registry for the future dial/replication path. The IPC version advances
to 24 so older clients fail closed instead of mis-decoding the new variants.

This adds no executable, listener, service, scheduled task, release build or
ZIP package. The provider continues to use the same runtime and dedicated Iroh
ALPN introduced by M0.9.62.

## 5. Remaining boundary

The next slice should exchange a small randomized subset of verified offers
over already authenticated peer sessions, with hop/age/count limits and no
social identifiers in the offer payload. It should then replicate one opaque
mailbox envelope to a small deterministic provider set and require independent
store-signed receipts before reporting the chosen durability policy.

Provider reputation, proof of free capacity, churn modeling, erasure coding,
private retrieval, Sybil resistance and resistance to colluding storage nodes
remain outside this milestone.

## 6. Verification

Network-free tests cover signature/tamper/expiry rejection, canonical import,
idempotent replay, monotonic replacement, expiry pruning, capacity bounds,
stable selection and endpoint-identity deduplication. Runtime projection tests
also reject Account/Device/conversation/mailbox-capability fields.

`scripts/verify-kilogram-volunteer-provider-selection-boundary.ps1` fails
closed if the limits, durable Redb state, monotonic replacement, deterministic
selection, transport-derived diversity, capability-free IPC or no-new-EXE
boundary disappears.
