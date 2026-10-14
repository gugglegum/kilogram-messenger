# RFC-0125: M1.1 threat model and security claims

Status: accepted for the M1.1 experimental baseline.

## 1. Purpose and scope

This document defines what the current Kilogram technical proof of concept is
designed to protect, which adversaries it considers, which guarantees are
actually supported by implemented controls, and which properties remain goals.
It is a review baseline, not a security audit or a production-readiness claim.

The scope is the current one-to-one messaging, account/device authority,
history recovery, Iroh transport, volunteer blind mailbox, local runtime IPC,
and build/evidence boundaries. Group chats, channels, a final cross-language
wire format, mobile platforms, anonymous multi-hop routing and production key
hardware are outside this baseline.

## 2. System and trust boundaries

The current system contains these principals:

1. an Account Root identity, normally offline except for authority changes;
2. one or more authorized Device identities and their local encrypted state;
3. peer devices participating in a conversation;
4. Iroh direct paths and relays carrying encrypted application traffic;
5. volunteer blind-mailbox providers storing opaque, capability-addressed
   ciphertext for bounded periods;
6. local desktop/bootstrap processes talking to the runtime through a
   loopback-only authenticated IPC contract; and
7. source, build and update infrastructure that distributes client code.

The principal trust boundaries are:

- **device boundary**: plaintext, live ratchet state and device private keys
  exist on an authorized endpoint;
- **Root boundary**: the Root key controls device authorization, revocation and
  recovery-policy authority, but does not sign ordinary messages;
- **peer boundary**: every remote object is untrusted until its version,
  bounds, signature, authority chain, recipient and session binding are
  checked;
- **transport boundary**: direct paths and relays are delivery mechanisms, not
  message-authority or plaintext-trust anchors;
- **storage boundary**: volunteer storage is assumed able to read all storage
  metadata and bytes it receives, but it receives no decryption key or social
  identifier by design;
- **local-process boundary**: the UI is not given Root/device/vault secrets and
  invokes the single runtime writer through a bearer descriptor; and
- **supply-chain boundary**: source review, pinned dependencies, CI and build
  attestations reduce risk but do not make a binary or dependency trustworthy
  by themselves.

## 3. Assets

The protected assets are, in priority order:

- message and attachment plaintext;
- Root, device, ratchet, mailbox-capability and local-vault secret material;
- authenticity and causal integrity of messages, acknowledgements, membership,
  device authority and recovery records;
- confidentiality of local history and projections at rest;
- integrity of the active device roster and revocation history;
- unlinkability of opaque mailbox records from Kilogram Account, Device,
  conversation and event identifiers at the storage API boundary;
- user intent during device linking, recovery and destructive authority
  changes;
- availability of delivery and history, treated as a best-effort property; and
- metadata such as IP addresses, timing, volume, peer relationships and online
  status, which is sensitive but not fully hidden by this version.

## 4. Adversary classes

The baseline considers the following adversaries independently and in partial
collusion:

- a passive network observer on one or more links;
- a malicious or compromised relay;
- a malicious volunteer mailbox provider;
- a malicious remote peer sending crafted, replayed, stale or oversized data;
- a Sybil attacker operating many relay or storage identities;
- an attacker who steals a device, compromises its OS, or obtains its unlocked
  process memory;
- an attacker who obtains the Root key or all factors accepted by a recovery
  policy;
- a local unprivileged process trying to invoke runtime IPC;
- compromised bootstrap, discovery, CI, dependency or update infrastructure;
  and
- a censor or failure that can delay, drop, reorder, partition or replay
  traffic.

The baseline does not assume a global passive observer can be defeated. It does
not assume independent volunteer identities correspond to independent people,
operators, networks or failure domains.

## 5. Current security claims

### 5.1 Message confidentiality and authenticity

Ordinary message plaintext is encrypted for authorized recipient devices.
Replicated events are signed and content-addressed; recipient-specific local
projections and ratchet state are not uploaded as plaintext. A relay or blind
mailbox provider that follows or violates the protocol should not receive the
message decryption key merely by carrying or retaining the record.

Device certificates, complete authority snapshots, membership and device-list
artifacts bind a message author to an Account and conversation. Session-bound
authorization is checked before event or inventory exchange. These mechanisms
detect modification and many replay/rollback attempts after a client has seen a
newer trusted revision.

These are implementation claims for the current PoC, not a formal proof. The
project has not had an independent cryptographic or implementation audit.

### 5.2 Relay and storage blindness

Iroh transport encryption protects traffic in transit, while application E2EE
keeps plaintext independent of the selected direct or relay path. A relay can
still observe endpoint, timing and traffic-volume metadata.

Blind mailbox providers receive pseudonymous capability keys, derived mailbox
and item identifiers, ciphertext length, retention time, request timing and
transport metadata. They are not intentionally given Account, Device,
conversation or event identifiers. Store-signed receipts detect some false
success claims; they do not force a provider to retain, return or delete data.

### 5.3 Account and device authority

The Root identity authorizes devices and permanent revocations. Device keys
perform routine messaging and scoped operations. Clients retain monotonic
authority observations and reject known rollback/equivocation, but a first
contact cannot prove it has the globally newest authority state without an
additional witness or trusted current-device path.

If one device is compromised, the attacker can read plaintext and secret state
available to that device and act with that device's capabilities until peers
observe a valid revocation. Revocation cannot make already copied plaintext
secret again and cannot prove that every disconnected peer has seen the newest
roster.

The recovery procedure is not privileged against an attacker holding the same
accepted factors. If an attacker controls the Root key, recovery phrase plus
required current authority material, or the required device quorum, they can
perform the same authorized recovery operation as the owner. A Root compromise
is therefore catastrophic until a separately trusted recovery/rotation path is
completed and observed.

### 5.4 Local process and storage boundary

The runtime is the serialized writer of protected state. The desktop client
uses a loopback-only, versioned IPC frame and a private bearer descriptor; it
does not receive the Root, device or vault keys. Local state uses an encrypted
vault and platform protection where implemented.

This does not defend against administrator/kernel compromise, injected code in
the runtime, malware running as the same user with access to the descriptor or
process memory, keylogging, screen capture, or an unlocked stolen device.

### 5.5 Availability

Direct, relay and replicated volunteer paths improve delivery opportunities,
but no current component guarantees availability. A censor, partition,
malicious peer, exhausted quota, unavailable relay, insufficient volunteer
set, or lost last history replica can prevent delivery or recovery. The default
volunteer limits are resource policy, not an SLA.

## 6. Threat and control register

| ID | Threat | Current control/evidence | Residual risk |
| --- | --- | --- | --- |
| T01 | Passive link observation | QUIC transport encryption and application E2EE | IP, timing, volume and connection graph remain observable at relevant vantage points |
| T02 | Malicious relay | Relay is not given application keys; peer/device signatures are verified end to end | Relay can correlate, delay, drop and selectively deny traffic |
| T03 | Malicious blind store | Opaque capabilities, E2EE payloads, bounded requests, signed store receipts and commit-before-delete | Retention/deletion cannot be forced; access correlation and denial remain |
| T04 | Crafted remote input | Size/count/time bounds, exact versions, signature/authority checks and fail-closed decode | Parser/logic bugs remain possible and unaudited |
| T05 | Replay or protocol downgrade | Session bindings, monotonic observations, domain separation, exact ALPN/version checks and the compatibility registry | No final negotiated cross-version protocol exists; first-contact freshness is incomplete |
| T06 | Compromised device | Scoped device capabilities, revocation, separate Root authority and per-device recipient slots | Live compromise exposes that device and permits valid actions until observed revocation |
| T07 | Compromised Root/recovery factors | Root is normally separate from routine messaging; quorum recovery policies exist | Possession of all accepted factors is indistinguishable from the owner |
| T08 | Authority rollback/equivocation | Complete signed snapshots, max-seen revisions, append-only evidence and conflict handling | Coordinated rollback before/without a newer witness and first-contact staleness remain |
| T09 | Sybil/eclipsed volunteer set | Admission work, bounded gossip/selection, authenticated observations and positive colocation avoidance | Operator/network independence is unproven; resource-rich collusion remains practical |
| T10 | Local IPC impersonation | Loopback-only listener, device-signed private bearer descriptor, bounded frames and one runtime writer | Same-user malware or descriptor theft can invoke allowed IPC operations |
| T11 | Disk theft | Encrypted vault and platform key protection on the implemented Windows path | Unlocked sessions, memory capture, platform compromise and non-Windows providers remain |
| T12 | Data loss/denial | Local histories, resumable sync, multi-source recovery and bounded mailbox replication | No global durability guarantee; all useful copies can disappear |
| T13 | Supply-chain compromise | Locked dependencies, source review, pinned CI actions, reproducibility/attestation evidence | Hosted builders, toolchains and dependencies are not fully trusted or independently audited |
| T14 | Group fork/order attack | None claimed in M1.1 | Group chats, MLS epochs, moderation and canonical presentation order remain future work |

## 7. Explicit non-goals and non-guarantees

M1.1 does not claim:

- anonymity against peers, relays, access providers or a global observer;
- HTTPS indistinguishability or resistance to traffic analysis;
- protection after endpoint or Root compromise;
- guaranteed deletion from recipients, backups or malicious stores;
- guaranteed delivery, censorship resistance or permanent history recovery;
- proof that selected providers are independent operators or networks;
- a production-stable public wire protocol or Rust API;
- secure group chats or channels;
- production support for non-Windows platforms; or
- independent security review, certification, bug bounty or SLA.

The accepted `M1.0.0` tag remains a Windows technical PoC. This RFC does not
retroactively strengthen its evidence.

## 8. Security-change discipline

Every change that alters a declared protocol identifier, version, signature or
derivation domain must update the machine-readable compatibility registry,
define reader/writer and downgrade behavior, and add migration evidence before
the old interpretation is removed. Silent reinterpretation of an existing
version or ALPN is forbidden.

Changes affecting keys, cryptographic transcripts, authority, recovery,
mailbox capabilities, parser bounds or metadata exposure require an RFC update
or successor and must state which threat/claim changes. A green unit test or CI
badge is regression evidence, not proof of a new security property.

## 9. Acceptance criteria

The M1.1 threat-model baseline is accepted when:

- this document and the compatibility RFC agree on scope and non-guarantees;
- the protocol registry validates every declared source binding;
- negative tests reject silent value drift, binding removal, path escape,
  unknown policy fields and undeclared wire-change claims;
- the closed readiness inventory includes the new boundary; and
- formatting and offline strict Clippy remain green without a network run,
  release build, archive or new milestone tag.
