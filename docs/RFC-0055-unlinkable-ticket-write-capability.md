# RFC-0055: Unlinkable opaque-ticket write capability

Status: implemented in M0.9.33.

## 1. Problem

M0.9.30 accepted any `PUT` carrying a non-zero generation. Recipient HPKE and
device signatures prevented forged bytes from becoming a valid connection
ticket, but a party that learned the pseudonymous channel could store an
arbitrary very high generation. The real publisher would then receive `409`
until the service record expired. Rate and capacity limits bounded service
resources but did not establish who may advance one channel.

A conventional authenticated account at the store would solve admission by
revealing a stable user identity and by introducing another account database.
This RFC instead makes the channel itself a self-authenticating, per-peer write
capability.

## 2. Scope and threat model

The capability protects `PUT` from an unrelated channel-aware writer. It does
not authenticate `GET`, hide the channel or access pattern, make a malicious
store available, stop volumetric DDoS, or replace recipient-side validation of
the encrypted publication.

The store still learns no Kilogram Account ID, Device ID, conversation label,
ticket, endpoint, prekey, or recipient identity. It sees a random-looking
Ed25519 verification key, its hash-derived channel, request generation, body
size, timing, and network metadata. The same publisher-device/recipient-account
pair deliberately uses one capability channel for its current endpoint ticket;
different recipient accounts and different publishing devices have unrelated
keys.

## 3. Capability derivation and distribution

The publishing client deterministically derives one Ed25519 signing seed with
the BLAKE3 `derive_key` KDF from:

- its protected 32-byte application Device signing seed;
- the exact recipient Account ID;
- the fixed context `kilogram ticket publication per-peer write capability v1`.

Temporary KDF input and output buffers are zeroized. The derived private key is
never serialized, put in the runtime repository, printed, sent to the desktop,
or uploaded. Deterministic derivation keeps the capability stable across
runtime restarts without adding another long-lived secret file.

Only the derived verification key is added to connection ticket v10. The whole
ticket, including that key and the allowed requester Account ID, remains signed
by the listener Device. An already enrolled recipient therefore learns the
right lookup capability from its out-of-band bootstrap ticket. Replacing the
peer account or the publishing device produces an unrelated capability and
requires the corresponding authenticated contact bootstrap.

This is a protocol pseudonym, not a Device ID. Although both use Ed25519 public
keys, the capability key is derived in a separate domain and is never accepted
for message, session, account, membership, or authority signatures.

## 4. Self-authenticating channel

The 32-byte channel is:

```text
BLAKE3("kilogram:ticket-publication-capability-channel:v1\\0" || write_key)
```

Consequently a requester cannot choose a different key for a known channel
without finding a BLAKE3 preimage. The service needs no registration request,
TOFU owner row, identity lookup, or non-expiring capability tombstone. This
also removes the first-write race that a server-side “remember the first key”
scheme would retain.

The channel is now scoped to the publisher device and recipient account, not
to an individual conversation. A runtime connection ticket is likewise
endpoint-wide and already authorizes that recipient account, so duplicating the
same endpoint across its conversations provides no additional routing value.

## 5. Exact PUT authorization

Every PUT includes two lowercase hexadecimal headers:

```text
X-Kilogram-Write-Key: <32-byte Ed25519 verification key>
X-Kilogram-Write-Signature: <64-byte Ed25519 signature>
```

The signed message is the unambiguous concatenation of:

1. `kilogram:ticket-publication-write-authorization:v1\\0`;
2. the 32-byte channel;
3. generation as big-endian `u64`;
4. body length as big-endian `u64`;
5. BLAKE3 digest of the exact HTTP body.

The store parses canonical lowercase hex, validates the Ed25519 key, recomputes
the channel from it, and verifies the signature before beginning the durable
generation transaction. A missing, malformed, wrong-channel, wrong-generation,
or wrong-body proof returns `403`. Only then do the existing atomic
`201`/`200`/`204`/`409` generation rules apply.

An observed valid request may be replayed byte-for-byte, but that is harmless:
the same generation and body are idempotent. Its proof cannot authorize another
generation, body, channel, or capability key. HTTPS remains mandatory remotely
so passive network observers do not receive either the channel or proof.

## 6. Shared narrow crate

`kilogram-ticket-publication` is the only implementation of:

- capability derivation;
- write-key and channel wire types;
- channel derivation;
- canonical signature encoding;
- PUT authorization signing and verification.

Both the client and the otherwise opaque ticket-store depend on this narrow
crate. The service still does not depend on account, identity, ratchet,
messaging, runtime IPC, state-vault, or Iroh crates and never decodes the HPKE
body.

## 7. Recipient validation and migration

Refresh derives the lookup channel only from the write key in the currently
signed contact ticket. After decrypting a publication, the recipient also
requires the replacement ticket to contain a write key hashing to that same
channel. Existing publication Account/Device/recipient signatures, expiry,
local observation high-water, authority checks, and atomic descriptor replace
remain mandatory.

Connection ticket version and signature domain advance from v9 to v10. This is
an intentional pre-M1 incompatibility: a v9 contact ticket has no authenticated
write key and cannot safely address the new store contract. Development peers
must exchange fresh v10 tickets once; message/event/session wire versions and
the Iroh ALPN do not change.

## 8. Verification

Regression coverage proves:

- deterministic restart derivation and distinct keys for distinct peer scopes;
- canonical key/channel/signature encoding;
- authorization binding to channel, generation, body length, and body digest;
- rejection of another valid capability attempting `u64::MAX` on a known
  channel while the legitimate generation remains readable;
- the real HTTP client sends the proof while uploading only the opaque HPKE
  envelope as its body;
- connection ticket v10 binds the expected capability key;
- two live runtimes still publish, fetch, install, replay, automate, compact,
  and restart through the production opaque store.

## 9. Honest limits and next step

Possession of the publishing Device secret permits deriving all of that
device's per-peer capabilities; normal device revocation and fresh contact
bootstrap are then required. A malicious service can always delete, suppress,
delay, or selectively return records. Public reads and source IP/timing/size
correlation remain visible, and per-IP limits are not Sybil resistance.

The next bounded work should choose a separate user-facing slice toward M1
rather than silently expand this rendezvous object into a message mailbox.
Privacy-preserving gossip/mailbox, endpoint failover across multiple devices,
and stronger distributed admission remain separate designs.
