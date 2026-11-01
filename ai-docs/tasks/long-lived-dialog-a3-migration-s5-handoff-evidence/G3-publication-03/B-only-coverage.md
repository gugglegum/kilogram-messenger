# B-only / no-intent coverage note after G3 attempt01

2026-11-01. Read-only review of accepted public evidence/current code; no additional process runs, private fixture access or product changes. G3's added A trigger does **not** close B-only liveness. Full pins and source comparisons are in `G3-repair02-B-only-coverage-01.json` (48861 bytes / `a72fe3273940e816a64c845422bbad18f18d9cf0a1e91bff17153e757e133715`).

## Accepted process coverage and its actual preconditions

The twelve control-loss cases use `live_l1::handoff::s53_control_loss_reopen_public`. Actual public assertions/root-success records show single-C, two-account fixtures, original one text per side, both current family1 and then family2 leases while still L1/NoLocalIntent/null controls/Q/cuts and unfrozen, before the **sole B request41**. A current ALPN2 opportunity was established and observed before B initiation. Current source: `migration_s53_handoff.rs:3020–3115`: shared setup45 seconds includes both current Locator1 imports, family1 warmup, Locator2 exchange and family2 warmup; the B-trigger/hit window is additionally bounded by first recorded A probe start +quiet15. This is stronger readiness setup than G3's Locator2 import followed immediately by a45-second Active poll.

Each row below has a root-success verification, actual outer exit0/cleanuptrue and public assertions rehashed in this review. All twelve actual assertions independently confirm both no-intent family setups and a lease-current B notification transaction. Historical acceptance is restricted to each controlled cut and does not imply new current process acceptance.

| Case | Accepted attempt suffix | Shared setup bound seconds | Actual B issuance since setup seconds |
| --- | --- | ---: | ---: |
| `F-s53-p-before-commit-01` |01 |45 |32.578328 |
| `F-s53-p-after-commit-01` |03 |45 |32.687003 |
| `F-s53-p-before-publication-01` |01 |45 |32.847444 |
| `F-s53-a-before-commit-01` |01 |45 |32.463147 |
| `F-s53-a-after-commit-01` |01 |45 |32.727224 |
| `F-s53-a-before-publication-01` |01 |45 |32.509706 |
| `F-s53-k-before-commit-01` |01 |45 |32.666087 |
| `F-s53-k-after-commit-01` |01 |45 |32.537969 |
| `F-s53-k-before-publication-01` |01 |45 |32.748663 |
| `F-s53-committed-before-commit-01` |02 |45 |32.625751 |
| `F-s53-committed-after-commit-01` |01 |45 |32.556143 |
| `F-s53-committed-before-publication-01` |01 |45 |32.707541 |

Representative P-after-commit03: actual first A probe31758ms; B issuance32.6870033s after setup; shared bound45s, quiet15. `family2_setup.after` at both peers has actual current family2, L1 and NoLocalIntent before the trigger. These observations prove B notification in a pre-admitted opportunity, not automatic creation of that opportunity within45 seconds after any B request. Later cut-recovery timers are separate (P-before-commit90; most others45) and must not be used as initial-discovery bounds. Latest Committed-before-publication acceptance explicitly records all12 cuts accepted in their respective reviewed scopes.

The accepted no-intent case `F-s53-discovery-integrated-01-attempt-01` uses `s53_discovery_preserves_ordinary_no_intent_public` (`migration_s53_handoff.rs:6270–6519`). Pair::fresh performs real legacy delivery/migration/Consent2; it then stops/restarts both current processes with trace, imports **both fresh Locator1** and Locator2, verifies L1/NoLocalIntent/null controls/Q/cuts, observes first ordinary delivery/ACK, a mutually current ALPN2 opportunity, matched resolution/L1 return and a second ordinary delivery/ACK. Its main observation loop is150 seconds, final ordinary drain30, controller230. Accepted actual A probe starts were30848ms and90923ms (spacing60075ms); root's limitation explicitly does **not** claim a second successful admission/resolution. Actual outer elapsed127.2356014s is historical scope evidence, not a current45-second startup guarantee.

## Current code identity and what remains open

For every one of those twelve cut captures plus the discovery capture, the four current files `runtime7/control/scheduler.rs`, `runtime7/network2.rs`, `runtime7/network.rs` and `runtime7/runtime.rs` match the historical captured source SHA256 values **exactly**. This includes A-only dialing, B notification on A-originated ALPN2, eligibility sampled before awaited ordinary admission, initial discovery5, interval60, probe whole45/quiet15, ordinary floor30/whole90. Parent fixture/setup and surrounding build scopes differ; matching these four files is not equality of the entire historical execution world or a new timing proof.

G3 attempt01 did not import/warm current Locator1 before requesting B-only C1 upgrade and did not observe current family2 at both C1 peers before that request. C1 remained L1/NoLocalIntent at Y and NotificationPending at X throughout540 captured status samples; C2 with A trigger reached Active after~33 seconds. That run's ordinary timeout messages are consistent with the source-derived delayed no-intent discovery mechanism; no trace establishes exact internal transition times or permanent nonconvergence.

The scheduler contract permits protected ordinary work up to90 seconds plus bounded admission/discovery behavior; it supplies **no universal45-second admission guarantee from a B request**. Therefore G3's former45-second initial Active precondition was stronger than the proven B-only process preconditions. It is not justified to call the observed45-second failure a protocol timing violation, nor to claim it was universally incompatible with every scheduler execution. A coverage gap remains for B intent issued before current ALPN2 readiness under the original G3 three-account/two-context startup ordering and its delayed initial ordinary path. The accepted single-C primed B-only tests and no-intent trace do not close it automatically.

Root's authorized G3 fixture now uses an explicit A trigger in each context while retaining X/B intent. Before whole-S5.3 acceptance, root must separately resolve whether the existing contract/evidence is sufficient for that open startup case or assign a bounded observable process check/architectural remedy. No new guarantee, scheduler fix, timeout relaxation or B-only acceptance is claimed here.
