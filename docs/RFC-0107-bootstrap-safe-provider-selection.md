# RFC-0107: Bootstrap-safe locally corroborated provider selection (M0.9.84)

Status: accepted locally after network-free verification.

## 1. Problem and claim boundary

M0.9.82 rejects cheap provider identities from new replica sets, while
M0.9.83 records whether an exact signed offer arrived through an already
Device-authenticated peer session. A hard observation requirement would,
however, prevent a new installation from selecting any provider before its
local social path has accumulated provenance.

M0.9.84 therefore introduces a preference, not another admission gate. The
goal is to use available local corroboration without turning bootstrap into a
deadlock or claiming that authenticated Devices are independent operators.

## 2. Selection policy

New replica-set, automatic legacy-upgrade and diagnostic selection apply the
following order:

1. Reject offers below the existing 18-bit admission-work floor.
2. Classify every remaining exact offer as locally corroborated when it has at
   least one authenticated observation, otherwise as bootstrap fallback.
3. Rank locally corroborated offers before bootstrap fallback offers.
4. Within each class, retain the existing salted deterministic rendezvous
   rank and exact transport-identity deduplication.
5. If the corroborated class cannot fill the request, fill every remaining
   slot from unobserved admission-qualified offers.

Observation count is deliberately binary for ranking. A second, eighth or
hundredth colluding Device cannot improve an offer beyond the same preferred
class reached by one authenticated observation. Thus Devices are not counted
as independent operators or failure domains.

## 3. Bootstrap, decay and compatibility

The preference threshold is one authenticated observation and the fallback is
always enabled. A fresh installation containing only explicit/manual offers
therefore retains the previous admission-qualified rendezvous behavior rather
than returning an empty set.

Provenance is bound to one exact short-lived signed offer. Replacement or
expiry removes it, so selection preference decays with the offer and must be
earned again for the replacement through authenticated gossip. It is never
carried forward merely because the store key or transport identity is reused.

Existing exact replica-set commitments continue to use indexed exact lookup;
they are not re-ranked or rejected for missing observations. Retrieval,
deletion, bounded gossip eligibility and the wire layouts are unchanged.

## 4. Privacy and security boundary

Authenticated runtime IPC remains version 26. Provider projections expose no
observer tag, Account/Device identifier, observation count or provenance row.
The authenticated local controller can observe which provider a requested
selection returned and therefore may infer that local preference affected a
result, but no remote peer receives that local state. Logs expose only the
constant policy name, threshold and fallback behavior.

This policy is not proof of operator, network or physical independence. One
operator can create provider and observing identities, and a compromised
social neighborhood can reinforce an eclipse. This follows the general
identity limitation in [John R. Douceur, *The Sybil
Attack*](https://www.microsoft.com/en-us/research/publication/the-sybil-attack/).
Admission work raises cost and binary local corroboration raises preference;
neither turns pseudonymous Devices into distinct people or operators.

## 5. Verification and stage boundary

Unit tests prove observed-first ordering, deterministic ranking, transport
deduplication, fallback completion, admission filtering and the absence of any
extra benefit from repeated observers. Runtime regression proves that a
corroborated candidate is preferred while provider IPC remains capability- and
provenance-free.

The fail-closed source boundary is
`scripts/verify-kilogram-bootstrap-safe-provider-selection-boundary.ps1`.
This stage is network-free and debug-only: it creates no release build, ZIP,
new executable, background service, field connection or external publication.

Local acceptance covered 17 mailbox-client tests, seven mailbox tests, three
focused CLI runtime tests, all 13 mailbox/provider/service-free static
boundaries, formatting and Clippy with warnings denied, using at most two
Cargo jobs.
