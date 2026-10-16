# RFC-0173: M1.1.47 canonical contact-ticket automation policy and status

Status: implemented as an inactive M1.1.47 candidate.

## 1. Scope

M1.1.47 specifies deterministic-CBOR local requests and Device-signed
responses for configuring and inspecting the existing per-contact connection
ticket scheduler. The two commands are `configure-ticket-automation` and
`ticket-automation-status`; their exact success results are
`ticket-automation-configured` and `ticket-automation-status`.

This is a local-control contract, not a new peer protocol. It does not activate canonical CBOR,
change active Postcard IPC version 27, change descriptor
signature domain v8, or alter any network ALPN. The existing runtime remains
the only component that may later execute ticket publication and refresh.

## 2. Policy identity and durable replacement

A configuration binds one enrolled contact, peer Account and conversation
label to:

- an enabled flag;
- a canonical root-only HTTPS publication service origin;
- a publication TTL from 30 through 3600 seconds;
- a refresh lead from 10 through 1800 seconds that is strictly smaller than
  the TTL;
- retry base and maximum delays capped at 3600 seconds; and
- a four-bit Ethernet, Wi-Fi, mobile and unknown-network permission mask.

The contact ID is not an independent caller assertion: verification rederives
it from the exact descriptor Account, peer Account and conversation label by
the canonical contact-ID algorithm.

The HTTPS origin is the opaque connection-ticket publication service already
used by the ticket publication protocol. It is not a central HTTPS mailbox and
does not store readable chat messages or volunteer mailbox ciphertext.

Generation one has no predecessor. Every replacement names the exact previous
generation and policy ID and advances once. The domain-separated policy ID
commits the Account, local Device, generation, predecessor, configuration
time and complete bounded configuration. The request ID is the durable
idempotency key: an exact retry may return `already-committed`; stale
predecessor or divergent request reuse fails before mutation. Thus the request
ID is a durable idempotency key with one exact semantic meaning.

## 3. Derived publish and refresh state

Every policy contains exactly two independently tracked actions in fixed order:
`publish` and `refresh`. Each action reports its scheduler state, last attempt,
last success, next attempt, consecutive failures, successful publication
generation, publication expiry and last result.

The optional fields form exact tuples and all present times and generations are
nonzero. Success binds the attempt time, generation and expiry; its next attempt
is exactly the expiry minus the configured refresh lead, but never earlier than
30 seconds after the attempt. Failure binds an attempt and positive failure
count; its deadline is the exact configured capped exponential backoff. The
scheduler state is derived rather than trusted as a free claim:

1. `disabled` when policy execution is disabled;
2. `network-blocked` when the observed network is not permitted;
3. `due` before an attempt or when its deadline has arrived;
4. `fresh` after success and before the next deadline;
5. `backoff` after failure and before the retry deadline; and
6. `publication-conflict-quarantined` only for refresh and without an automatic
   retry deadline.

Status returns at most 256 policies in strict contact-ID order. Its
domain-separated snapshot ID commits the Account, local Device, observation
time, network class and complete ordered policy projection.
The signed observation time must remain inside the authenticated request
validity window; a future or stale status cannot be attached to that request.

## 4. Execution and privacy boundary

Configuration and status are foreground runtime operations: they require no
restart, execute only while the messenger is running and explicitly report
that no OS background service is enabled. Closing the messenger stops the
scheduler. This contract does not
register Windows Task Scheduler, a Windows service, a Linux daemon or any other
independent background process.

Routine IPC never carries ticket plaintext, HPKE envelope bytes, publication
read or write capabilities, Account Root secrets, Device private keys or local filesystem paths.
A successful HTTPS action remains only a Device-signed report
of one exchange; it does not prove future remote availability, peer
observation, anonymity or message delivery.

## 5. Verification and activation

The Rust implementation enforces deterministic CBOR, object-size limits,
strict HTTPS-origin normalization, exact context binding, Device signatures,
derived state and fail-closed unknown variants. Checked-in golden vectors are
reproduced by an independent Node implementation of CBOR, BLAKE3 and Ed25519.
The request and signed response decoders are included in the bounded mutation
corpus.

M1.1.47 changes no active local control and no peer network wire. Removing this
item from the completion ledger leaves two explicit canonical-target gaps:
endpoint-announcement bootstrap-helper disposition and removal of the legacy
central-HTTPS mailbox commands.
