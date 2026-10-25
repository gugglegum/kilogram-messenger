# Narrow repair after measured diagnostic baseline

Root authorized the comparable before/after diagnostic and these two repairs;
another full P and publication remain forbidden until separate review.

## Owner eligibility

Baseline diagnostic:17874 unconditional `repository.installed(c)` calls,
21.0315975s, average1.1767ms. This is19.4% of108.4893818s summed disjoint owner
service across two processes. The method reads/decrypts the spec value through
`store.get`; it is not an in-memory boolean. Native forks598 cost0.059887s and
packet Native encrypt/decrypt combined0.0392365s, so those were left unchanged.
Status75ms and its guard projections were also left unchanged.

The repaired loop removes only the unconditional installed lookup/skip block.
Context round-robin selection, alternating receive/materialize, batch<=4, yield,
2ms idle sleep, per-turn error retry and recovery checks remain at their old seams.
Startup and recovery still enumerate installed contexts and restore controls/Native.

Existing nonempty-work guards are the authority:

- `materialize`: context bound; authenticated in-memory root queue count with poison
  check; zero=>no work. Nonzero=>require(false), installed snapshot decode, fresh
  current policy/provider/time, active/not-blocked, actual queue row checks, trial,
  closed commit's final policy gate. No local lease is required for genuine offline
  encryption, as required by the contract.
- `drain`: context bound; poison-checked authenticated pending/head ledger; empty=>no
  work. Nonempty=>require(true), installed snapshot/current policy/time and actual
  lease; active/not-blocked; exact pending/frontier/parents/Seen; actual Native trial;
  final lease/current gate and atomic readable/receipt/credit commit before ACK.
- Poison/reopen handling remains before and after each data turn. No long-lived
  installed/policy/read cache was introduced. Counters come from the current accepted
  root/ledger, and actual work rereads its installed snapshot.

## R1 route restriction

`RouteConfig2::effective` now requires loopback for every policy with an IP bind.
The previous condition incorrectly exempted Auto+explicit relay. RelayOnly still
forbids a bind. Validation precedes construction of the endpoint in `bind`.

The added pure configuration regression accepts IPv4/IPv6 loopback Auto+relay,
rejects wildcard/LAN/global/link-local addresses before bind, and retains
DirectOnly/Auto-without-relay/URL-only RelayOnly and invalid mixed/empty cases.
Existing F12 signed-locator/actual Minimal/admission checks remain separate.

## Measurement and review status

The first diagnostic is immutable: `diagnostic-costs-64-duplex-1` and
`diagnostic-attribution.json`. It is not P acceptance. The comparable second run
uses a new fixture, the same98 Text+4 genesis, watchdog280, unchanged polling75ms,
same verification/locked/offline/Jobs12/BelowNormal settings and opt-in numerical
timers. The removed OwnerInstalled category has zero per-turn calls after repair;
it is omitted from the new enum/report rather than replaced by a fake counter.

Comparison and selected guard/F12 verification are pending. Old P4 remains FAIL;
this repair does not retrospectively turn its partial observations into PASS.
