# Usable peer chat: revised outcome and review handoff

2026-11-21 revised candidate, baseline `7d808f70ee5ee7d6f66a97d9195e0af9dc9872e1`. Addresses [root review](usable-peer-chat-root-review.md). Independent implementation/local verification complete; physical LAN and genuinely different-network acceptance **BLOCKED / NOT EXERCISED**. No commit/push before review. This section is authoritative for the revised candidate; the initial candidate below is preserved history.

## Revised implementation

**Continuous ownership:** existing `Setup::open_terminal` is retained before protected network-state reads, endpoint creation, link fetch and confirmation; `create_account` reuses it. The same Setup moves into `Chat::from_owned_setup`, with the existing coordinator over private `std::io::pipe` streams and an owner thread. No unlock/reopen or lock bypass. `SetupOwnership` retains an `Arc` of the SAME locked File object in Local, including after failed/closed owner start while control work remains alive. Ordinary constructors still acquire the original exclusive OS lock. The default administrative terminal/monitor path stays unchanged.

**Human waiting:** no120-second confirmation deadline. One pending candidate is confirmed by the UI outside control requests, so retries neither hold a human request open nor repeat prompts. Network I/O remains bounded; accepted account callbacks are joined rather than detached by an outer timeout around human mutation. Existing bounded helpers, signed-consent expiry and identity/context checks remain authoritative. Lost replies do not erase already accepted signed progress. `/quit`, EOF and Ctrl-C stop waiting promptly; initial network waiting is cancellable. Typed user closure/confirmed helper cancellation is a normal exit only after independent cleanup gates. Real errors and `CleanupUnconfirmed` remain errors.

**Locator churn:** one active temporary incoming stage, removed on normal/refused/cancelled completion; no new permanent `network-{digest}.json`. Explicit product `ImportLatest` retires only the exact superseded hash-bound retained native file after valid retention. Ordinary/manual Import preserves its evidence semantics. Invalid payload does not change the accepted route/record. Fresh exports are removed after copying bounded public bytes. Cleanup uncertainty stops with an error; locator task completion/cancellation is acknowledged at shutdown. Historical artifacts are not scanned/deleted. Abrupt termination during staging can leave an unimported temporary file; no general power-loss/orphan cleanup is claimed.

The broader architecture remains the initial candidate's first-dialog/runtime2/E2EE/durable-intent reuse, separate bounded Iroh TLS control channel, DPAPI technical state, explicit fresh Internet route and actual live direct/relay reporting. No runtime7 rewrite, server deployment or Windows/VPN/firewall/global policy changes. Public Iroh lookup/relay remains an explicit metadata/availability dependency, not an account/history server. The invitation authenticates its endpoint/public account binding, not a real-world name.

## Revised actual checks

Evidence root `.tmp/usable-peer-chat`:

- `revision-checks-06`: stable first-dialog3 + peer6 + legacy monitor4 = **13 passed**, exit0, empty/nonforced Job. Native same-file test retains the lock after Chat drops and releases only after the final ownership guard. Actual Session/native-runtime churn:16 valid signed updates +32 invalid shape/context updates, one retained file, rejected route unchanged, graceful/reaped/EOF stops. Fixture helper images are separately pinned in `images.json`.
- `kit-functional-02`:40 bidirectional messages, B offline, A pending2, ordinary same-profile A restart, original event IDs/history BEFORE B return, automatic delivery, equal histories and exactly42 ordinary incoming notifications. Actual MAC-corrupt display cursor refusal/no silent fallback and explicit authenticated `/resync` pass. Re-pairing original disposable identities preserves all43 prior events, performs no bootstrap/recovery repeat, allows the previously joined profile to invite its original peer, and ends with45 unique equal events/roles/text on both sides. This fixture was already Internet-configured, not a physical old LAN pilot.
- Actual existing-account confirmation held **125.014 seconds**: real frontend duplicate refused; real legacy owner returns `connect_failed`; one prompt; no preapproval setup/network-state mutation. EOF cancels normally without changing that existing profile. Pairing then completes on the same accounts (`confirmation-ownership.json`).
- Final product directories each have zero staging, zero `network-*`, zero fresh public exports and one retained peer file (`bounded-locator-files.json`).
- `kit-probe-02`: actual ordinary PS5 receiver/final images,20 authenticated echo RTTs, p95 **643.610 ms**, maximum1542.953 ms, stop acknowledgement/ordinary cleanup. Local fixture; RTT includes both directions and processing, is NEVER halved or called physical one-way latency.
- Both final product outer receipts: BelowNormal,900/930 seconds, exit0, empty/nonforced Jobs. No personal profiles or newly injected runtime crashes.
- `launcher-check-05.json`: native CMD/CRLF `/quit` before account creation, exit0/no stderr/no profile creation, all manifest files match. Personal launcher never executed. `kit-refresh-02.json` verifies only own static kit files changed after no processes were using those app paths; no windows stopped or profiles copied.

| Final local measurement | Result |
|---|---:|
| Samples |40, one PC/two processes |
| p50 input flush to peer stdout |260.977 ms |
| p95 |**443.759 ms** |
| Maximum, not discarded |1116.512 ms |
| Initial two-account creation/pairing/connect, separate |10.169 s |

Same-driver `perf_counter`; no host-clock subtraction/compositor claim. Local p95 target met, home-LAN target unverified. Historical before: `local-03`, n6, p50 4113ms/p95 8029ms. The presentation/owner polling and crypto timing diagnosis below remains applicable; differing sample sizes/cohorts are explicit.

## Revised source-bound kit

Build06 compiles successfully with unchanged source, but its strict receipt fails because own MSVC `VCTIP.EXE` remains after LINK and needs scoped cleanup. PID/path/start are retained. Compile/check preparations01–04 and compile-only attempts with forced cleanup remain failed strict receipts, not clean test PASS. A process-local `VSCMD_SKIP_SENDTELEMETRY` experiment was ineffective and removed; no global setting changed. Build07 validates the SAME three image hashes/exact source via Cargo without relinking: exit0/empty/nonforced. Native product/test Job predicates have no compiler-process exception.

`build-final-07/build-receipt.json`: locked/offline verification-scale, Idle/jobs1; source inventory SHA256 `029450500114470c7e87924c0130515db1e82b16d1af2cd7c5b973edb352e96f`.

| Final image | SHA256 |
|---|---|
| `kilogram-peer.exe` | `e6e27dd3b0057dbd6fbd2a1c57507418fbe5efdefd826b4baf60f2092e5fd6d6` |
| `kilogram-cli.exe` | `065d45e6affae60a58ab2ca6a1116c152b81e868d807aa0075dc463f90ac6a29` |
| `kilogram-bootstrap.exe` | `9838a5b97e6550d852b306b6b892d88df94551e295bffa5add61db100f0c44a9` |

Final ZIPs under `kit-final-05` (12 static entries each; no profiles/words/history):

- `Kilogram-PC.zip`: `8846851c4b2d1358aab2135d3db8dc91422b8f9cbc45aaec8d5e4e864af76182`.
- `Kilogram-Laptop.zip`: `fb6d0a9aa6b0282e1f5d77b7f796c6cf3d364d85715b727ba0c0703015e23ad0`.

Stable ordinary PC entry: `.tmp/usable-peer-chat/manual-kit-pc/Start-Kilogram.cmd`. [Guide](../usable-peer-chat-guide.md). Personal `.tmp/manual-pilot-pc/profile` and laptop `manual-pilot/profile` remain untouched; `Open-existing-pilot.cmd` not executed. New default kit profile is still absent; automated data are separate. `review-02` binds exact36 raw/blob/snapshots, scoped diff, current full source, native sources, independent histories, ZIPs and selected evidence; root notes/foreign changes excluded, index empty. Review01/failed preparations remain preserved.

Strict Clippy remains **not green**. The19 first-dialog/39 terminal findings and diagnostic-only frontend counts below describe the original candidate, not a fresh strict PASS for this revision. No lint allowances or new broad lint campaign were used. Revised scope passes compilation, focused native checks, formatting and scoped diff checks.

## Current external prerequisite

The permitted single new read-only probe `network-prerequisite-03/result.json`, **2026-11-21 09:43:06 Asia/Yekaterinburg**, gets exit255: `Connection closed by 192.168.0.152 port22`. No authenticated readiness/identity obtained. The prior timeout is preserved. Current address ownership, laptop power/network and user readiness are not established. Nothing deployed there, no timed human test started. Physical LAN/NAT/different-network operation remains NOT EXERCISED.

Root requested user readiness. One bundled continuation: reachable laptop plus ordinary desktop, unpack final laptop ZIP once at stable `C:\Users\test\KilogramLab\usable-peer-chat`, open `Network-test.cmd`, paste ONE invitation from the PC bounded probe's evidence `stdout.log`. Replies/stop use disposable data automatically. Exact PC command is in the historical handoff below; use fresh physical evidence, `-Network LAN`, then genuinely move laptop to another network and repeat fresh `-Network different-networks`, preserving PC Internet/VPN. Collect both receipts and factual direct/relay/network observations; labels alone are not proof. SSH logon is not the ordinary DPAPI/bootstrap context; existing pilot reuse requires its original Windows user.

MVP gaps remain Windows/interactive context, one peer/profile, terminal, initial cold pairing about10s; consent renewal UX, multi-contact, GUI/groups/channels/mobile/portable recovery unfinished. Historical field05 does not prove power loss, arbitrary-crash UI behavior of this frontend or general Internet readiness. No next stage/publication before review.

## Historical initial candidate (superseded by the revision above)

Baseline `7d808f70ee5ee7d6f66a97d9195e0af9dc9872e1`; authoritative task: [usable-peer-chat-outcome.md](usable-peer-chat-outcome.md). Prepared for root review on 2026-11-21; not committed or published. The complete kit passes local public-path functional/recovery/latency checks. Physical LAN and different-network acceptance are BLOCKED by the laptop prerequisite below, not passed.

Source assessment: terminal setup exposes the first-dialog artifact/consent state machine as manual files and preview IDs. `Chat` uses first-dialog's owned runtime2, durable send-intent recovery, authenticated history and typed shutdown. FreshRoute2 restricts the existing public route to loopback or explicitly selected private LAN; general Iroh N0 discovery/NAT/relay support elsewhere in the repository was not serving this path. Indexed runtime7 remains outside this product change. The chat owner samples status/pending at one-second intervals; the terminal history monitor also has one-second idle and two-second empty-history retry periods. These are candidates, not a measured explanation of the reported four seconds. Runtime2 already has opt-in numerical timing counters (`KILOGRAM_TEST_OD2_TIMINGS`).

Chosen structure: `kilogram-peer` is a small separate product frontend; `kilogram-cli talk` delegates to it. This keeps repeated UI builds independent of the 43000-line administrative main. The kit contains peer/CLI/bootstrap internally, with a single user launcher. Peer exchanges bounded PUBLIC artifacts and signed locator bundles over a separate Iroh TLS control channel. Existing first-dialog signing/import, context/identity bindings, owned chat/runtime, send-intent recovery and E2EE stay authoritative. Control traffic does not carry message plaintext, account private keys or recovery words. User confirms the peer account once; invite is a bearer capability plus endpoint/artifact fingerprint, not proof of a person's real-world name. Actual signed consent/runtime authentication remains required.

New opt-in fresh Internet route: Auto, wildcard IPv4 OS-assigned bind and an explicit canonical HTTPS relay. The original shared/indexed RouteConfig2 loopback restrictions and explicit direct-LAN path are preserved. The runtime retains a direct path while relay registration retries; no public server deployment/account server/history service is added. Default relay comes from the pinned Iroh1.1.0 EU production configuration. Control uses Iroh N0 lookup/rendezvous and retains peer addresses for reconnect. Relay/lookup metadata and availability assumptions must be stated in final guide. Real physical different-network proof remains required or must be explicitly blocked; LAN/local emulation will not be substituted.

`peer-network.sealed` stores the separate stable technical endpoint key, invitation capability and selected endpoint under current-user DPAPI on Windows. The profile's existing exclusive lock is acquired before opening its network state or endpoints; explicit capability/shape/relay/identity checks precede mutation. The joiner initiates the periodic exchange and the inviter responds; at most two bounded incoming control connections avoid an incomplete TLS attempt blocking all admission. Account mutations are serialized; nonblocking lock acquisition keeps consent work off the async scheduler. These focused checks and tests are not a general security audit. Windows Job containment for the product frontend uses the safe pinned win32job2.0.3 wrapper (primary API/source reviewed), containing its private chat/runtime helpers with kill-on-close. Normal cleanup passively drains and proves no descendants before disarming it. No unsafe Rust is introduced in product crates.

Resource/test discipline: Idle/Jobs1 via cargo-resource-policy.ps1. Compile-only cargo checks/builds; network Rust test harnesses run through run-cargo-tests-stable.ps1 at stable filenames. A small ordinary `-Bin` selector was added to the existing runner to avoid compiling unrelated giant integration harnesses when testing the peer frontend; frozen/S54 lanes remain closed. No new general test orchestration system is intended. Disposable product profiles only. Personal pilot profiles `.tmp/manual-pilot-pc/profile` and `C:/Users/test/KilogramLab/manual-pilot/profile` and their possibly open windows have not been touched.

## Measured correction

Evidence root: `.tmp/usable-peer-chat`. All send/display timings use one driver's `perf_counter`: input flush through receiving stdout. No subtraction between host clocks, no compositor/pixel timing. Compilation is kept separate from latency measurements.

| Run | Samples | p50 | p95 | Maximum | Scope |
|---|---:|---:|---:|---:|---|
| `local-03` | 6 | 4113 ms | 8029 ms | 8029 ms | One PC, old owner/monitor cadence |
| `local-05` | 40 | 259 ms | 389 ms | 1256 ms | One PC, corrected path; intermediate helper images |
| `kit-functional-01` | 40 | 260 ms | 397 ms | 1242 ms | One PC, all three final source-bound images, stable kit path |

`local-05` account/pair/connect took 8.07 s; final `kit-functional-01` took 8.02 s, measured separately. These are not physical LAN results. The final local p95 meets the engineering threshold; the two roughly one-second outliers remain in the sample rather than being discarded. The home-LAN p95 target is still unverified.

The multi-second cause was reproduced and localized: `Work.action()` invalidated its presentation view; `owner_scope()` then waited for another one-second status publication for each read/send. The monitor added one-second idle, 250-ms scheduling and two-second no-history waits. `local-03` numerical counters showed encrypt/decrypt maxima below 0.22 ms, materialization around 8–10 ms and queue/ingest around 4–6 ms; cold commits were larger. Crypto was not the source of the four-second cadence.

The owned immutable runtime/context/role scope now survives presentation invalidation. Scope drift cancels the owner; this does not cache permission or revive a connected badge. Actual operations still check the retained child, pinned descriptor, scope and current runtime permissions. A production `Work` test covers late generations, immediate scope reuse after action, no badge revival and drift refusal.

The product has a separate pinned read-only observer, with 100-ms incremental history follow. It reads only new ranges and checkpoints after stdout flush, including messages which raced startup. Opaque checkpoint parsing is not authentication: storage verifies MAC/database/view/context bindings. Invalid checkpoints stop automatic display; explicit `/resync` performs authenticated replay without resending/deleting messages. A crash between output and checkpoint can repeat a page; exactly-once display across arbitrary crashes is not claimed. The administrative terminal's original monitor remains the default.

Fast next sends exposed the old one-second reconciliation of a previous protected intent. The worker now reconciles it on demand, without the new command's progress observer, so an old receipt cannot be attributed to the new request. Only explicit pre-admission `pending_request` is briefly retried with unchanged business request ID/text. Uncertain, admitted and conflicting requests are not automatically resubmitted.

## Functional / negative checks

`local-05` passed 40 bidirectional messages, receiver offline, two pending messages, same-profile ordinary sender restart, original event IDs/history before receiver return, automatic delivery, exact equal histories, unique events and exact ordinary incoming notifications across restarts. Duplicate-profile refusal and real native rejection of a MAC-corrupted display cursor also passed, with no silent fallback and explicit `/resync`. Frontends and outer native Job closed empty/nonforced. See `history.json`, `offline-recovery.json`, `notifications.json`, `checkpoint-refusal.json`, `result.json`, `outer.json`, raw JSONL and timing files. Personal profiles were not used.

Preserved failures: `local-01` bootstrap generation mismatch (correct protection refusal); `local-02` still-local-only locator envelope; `local-04` fast second send blocked by the retained intent. `local-03` is functional PASS / latency FAIL. All ended with empty outer Jobs and no forced cleanup.

`probe-local-01` failed preparation because PS5 read a BOM-less Cyrillic script as a legacy encoding; no B account was launched. Corrected receiver PS1 is UTF-8 BOM. `probe-local-02` passed 10 actual PS5 receiver/product echo round trips and ordinary stop receipt/cleanup, all native Jobs empty/nonforced. RTT p95/max was 2371 ms (an early outlier); most samples were 0.28–0.58 s. RTT includes both directions and receiver processing, is never divided by two, and does not replace a one-way or physical-network measurement.

Current minimal-lock stable-runner units passed: route/selected-relay refusal, opaque checkpoint shape/resume, production scope/freshness, invitation/capability, cancellation, DPAPI corruption and honest uncertain-send UI. Four legacy monitor regressions also passed after sharing terminal modules through the library. Network Rust harnesses were not launched by direct `cargo test`.

The lockfile was minimized: all pre-existing packages/versions/dependency edges match HEAD; only CLI direct dependencies and `win32job` were added (`lock-scope-fix.json`).

Strict Clippy is **not passed**: expanded checking reaches 19 pre-existing first-dialog findings; the final strict entry-point check exits 101 on 39 pre-existing terminal expressions. An earlier run also found one new initialization unwrap; it was removed by initializing state before publishing its mutex. Original failures and HEAD-expression comparison (`lint-summary.json`) are retained. An explicitly diagnostic `--cap-lints=warn` collection reached the frontend and exposed one redundant conversion, which was fixed; the final frontend diagnostic count is zero. Diagnostic collection is not a strict PASS. No lint allow attributes or product/security checks were disabled to manufacture a green gate.

## Final image / kit evidence

`build-final-02/build-receipt.json` proves all three optimized images were rebuilt from the same unchanged source inventory, using the minimal lock, Idle priority and one Cargo job. Exit 0, empty native owned Job, no forced cleanup. Source-inventory SHA256: `cc04db059894a78c723df6f3b5d264c135220ca81903c16d2c594403d15c5f6e`.

| Image | SHA256 |
|---|---|
| `kilogram-peer.exe` | `33db4a89c6903789a2535b8d52b97b1fd21402165c071d60167d65fa7e772d88` |
| `kilogram-cli.exe` | `ee93f69831982a25188ecbf35eae1c5a53dfa4096b85975a6dd63f76672950dd` |
| `kilogram-bootstrap.exe` | `9838a5b97e6550d852b306b6b892d88df94551e295bffa5add61db100f0c44a9` |

`kit-functional-01` ran those exact images at the stable `manual-kit-pc/app` path. It passed the 40 bidirectional samples, two offline pending messages, same-profile sender restart before B return, original event IDs, automatic delivery, exact histories and ordinary incoming notifications. It also passed duplicate-profile refusal, actual native cursor-MAC refusal and explicit authenticated `/resync`. Finally the same original disposable accounts were paired again after archiving only their technical network-state files: no bootstrap/recovery repeat, the formerly joined profile could invite its original peer, all 43 prior events remained identical, and both final histories contained the same 45 unique events after two new sends. These profiles already used the new Internet route; this does not prove migration of a physical old LAN-mode pilot or authorize touching the user's profiles.

`kit-probe-01` exercised the actual PS5 ordinary-user receiver with the final images: 20 authenticated echo round trips, p95 673 ms, maximum 2474 ms. This is causal round-trip time including both directions and receiver processing; it is never divided by two or called physical LAN latency. Stop acknowledgement, ordinary frontend exits and empty/nonforced Jobs passed. Raw JSONL, numerical timings, histories, inputs and strict outer receipts are retained for both final runs.

Portable final bundles are `.tmp/usable-peer-chat/kit-final-03/Kilogram-PC.zip` and `Kilogram-Laptop.zip`. Each contains three EXEs, internal launch/test helpers, checksums and the short guide; no profiles, recovery words or histories. The ordinary PC launch path is `.tmp/usable-peer-chat/manual-kit-pc/Start-Kilogram.cmd`. `Open-existing-pilot.cmd` is prepared but has NOT been executed. Source batch templates are normalized to UTF-8 CRLF in the packager independently of Git checkout settings. Actual CMD launch `launcher-check-04.json` passed with `/quit`, exit 0, no stderr, matching manifest files, and no profile creation. Earlier failed encoding/line-ending launcher preparations are preserved; they are packaging failures, not E2EE failures.

Review inventory: `.tmp/usable-peer-chat/review-01` records the exact 34 own paths, raw/blob/snapshot bindings, scoped diff, final build-source equality, kit hashes, selected raw evidence hashes and current foreign-path hashes. The index stays empty; root task/review notes and unrelated changes are excluded. No commit/push before review.

## Physical-network prerequisite and one bundled handoff

The latest bounded read-only SSH probe (`network-prerequisite-02/result.json`, 2026-11-21 08:23:55 Asia/Yekaterinburg) timed out at `192.168.0.152:22`, exit 255. Current laptop power/network/address is not established. Nothing was deployed there. Physical LAN and different-network communication remain NOT EXERCISED. Public relay HTTPS reachability alone does not prove NAT traversal or message delivery.

The prepared laptop kit should be unpacked once at a stable path, for example `C:\Users\test\KilogramLab\usable-peer-chat`, preserving the separate manual-pilot profile. A human must provide the laptop's reachable address/ordinary logged-in desktop and, for the second run, move it to a genuinely different network (for example a phone hotspot). PC Internet/VPN stays unchanged. SSH can transfer the authorized kit when available, but SSH logon alone is not the ordinary interactive DPAPI/bootstrap context.

For each fresh physical case, run the existing bounded PC probe from the repository (ordinary NativeToken, BelowNormal, 900/930 seconds):

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-usable-peer-chat.ps1 -Exe .tmp/usable-peer-chat/manual-kit-pc/app/kilogram-peer.exe -Evidence .tmp/usable-peer-chat/physical-lan-01 -Samples 20 -Probe -Network LAN
```

The invitation is written to that evidence directory's `stdout.log` as soon as A is ready; copy that ONE line into the laptop's `Network-test.cmd` opened on its ordinary desktop. Responses and shutdown are automatic on separate disposable profiles. Collect the laptop `app/test-data/.../result.json` plus PC raw/outer receipts and actual network/path observations. For the second physical case use a fresh evidence path and `-Network different-networks`; the label alone is not proof of network separation. Both hosts' factual route/network observations are required. A physical RTT below 500 ms bounds either causal leg, but a larger RTT cannot determine one-way latency and must not be halved. No physical probe is started while this prerequisite is unresolved.

MVP gaps remain explicit: Windows/ordinary interactive account context, one chosen peer per profile, terminal UI; consent renewal, multi-contact management, GUI, groups/channels, portable recovery and mobile devices are unfinished. Historical field05 remains process-crash-after-durable-ack evidence, not power-loss or general Internet readiness.
