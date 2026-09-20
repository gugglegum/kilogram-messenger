# RFC-0105: Sybil-costed volunteer provider admission (M0.9.82)

Status: accepted locally after network-free debug tests, Clippy and the
fail-closed mailbox/provider boundary suite.

## 1. Problem and honest claim

The current registry deduplicates store offers by Iroh endpoint identity. That
prevents one endpoint from being counted several times, but an operator can
create more keys and endpoints. Self-signed identities alone cannot prove that
two providers have different operators. This is the fundamental boundary
described by Douceur's *The Sybil Attack*: an open distributed system needs an
external scarcity or trust assumption to limit identities.

M0.9.82 therefore does not make Kilogram Sybil-proof. It adds a bounded,
cheap-to-verify cost for every fresh provider identity/endpoint offer. This is
a resource-cost floor in the Hashcash family, not a human-identity certificate
and not evidence of physical independence.

Primary references:

- [John R. Douceur, The Sybil Attack](https://www.microsoft.com/en-us/research/publication/the-sybil-attack/)
- [Adam Back, Hashcash - A Denial of Service Counter-Measure](http://www.hashcash.org/papers/hashcash.pdf)

## 2. Admission-work contract

A new volunteer store offer is generated with at least 18 leading zero bits in
a domain-separated BLAKE3 digest. The digest binds all signed offer content:

- store key and complete provider endpoint;
- policy class and capacity bounds;
- issue/expiry times;
- the offer nonce.

The Ed25519 signature is created only after the bounded nonce search succeeds.
Verification performs one digest and the existing signature check. Generation
accepts difficulty from 1 through 20 bits and stops after at most `2^24`
attempts; the runtime default is exactly 18 bits. Higher work does not improve
selection rank, avoiding a permanent proof-of-work auction.

The proof expires with the short-lived signed offer. It cannot be moved to a
new store key, endpoint, capacity statement or lifetime without redoing the
work and signature.

## 3. Compatibility and activation

The signed offer wire layout is unchanged. The registry may retain and resolve
an older low-work offer so existing exact replica-set commitments remain
resolvable. This is required for recovery and deletion of already stored opaque
items.

Low-work offers are excluded from:

- new exact volunteer replica-set selection;
- automatic legacy-to-v2 capability rotation;
- diagnostic provider selection;
- further authenticated provider gossip.

New embedded stores generate qualified offers by default. Selection continues
to require distinct transport identities after the admission filter. No
Account, Device, conversation or mailbox capability is added to the offer, and
authenticated runtime IPC remains version 26.

## 4. Security boundary

An attacker with faster or parallel hardware can still generate many offers.
The fixed puzzle does not establish fair cost across desktops and mobile
devices, identify colluding operators, prove advertised capacity, or prevent a
well-resourced eclipse attack. It also does not prove that two providers have
different operators, networks or physical failure domains.

The next diversity slice should add local authenticated observation provenance
without transmitting a user's social graph, then require an explicit policy
over distinct observations/failure domains. A later field run must place the
selected stores on physically and operator-independent hosts. Those layers are
additional assumptions; they must not retroactively relabel this cost floor as
complete Sybil resistance.

## 5. Verification and stage boundary

Unit tests cover bounded generation, cheap verification, content binding,
low-work compatibility retention, exclusion from new selection and exclusion
from gossip. Runtime tests prove that low-work offers remain visible in the
bounded registry but cannot enter a new replica set.

The stage is network-free and debug-only: it runs no field connection, release
build, ZIP creation, background service or external publication. The static
boundary is `scripts/verify-kilogram-provider-admission-work-boundary.ps1`.
