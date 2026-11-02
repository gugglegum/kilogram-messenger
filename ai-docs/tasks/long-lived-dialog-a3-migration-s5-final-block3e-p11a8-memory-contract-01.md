# P11A8 bounded memory observer — proposed contract

2026-11-02. Baseline `546487d9c8001c6d110444a05acb3c0d1c381772`, tree `7c6f4ae6d5dd0a599390506268e7891686fb32db`. Task5558B/SHA256 `d9ac453a446ae415c3ce8a73797241d5402c90e56f9ed913131ccc19e6f64014` verified. P11 section6 and accepted A7 apply. Contract only: no Rust/build/tests, memory-shell/messenger/private-fixture/network execution, commit or push. Whole S5.4 remains gated.

## Source map and coverage

| Actual seam | Proposed hook |
| --- | --- |
| [root spawn/wait_file/OwnedChild](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/migration_s5_process.rs:133), private_bytes:172, old_pair:536 | Register immediately after spawn, before readiness; progress in alive/waits; pre-kill and terminal taps. Old private_bytes is synchronous PATH/PID-only and unsuitable. |
| [Node::start_controlled/status](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s5_l1.rs:174), [Pair::fresh/start/stop/Drop/wait](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s53_handoff.rs:294) | Separate spawn path; register once, not again in its wrapper. Pair::fresh is legacy, not canonical S54; non-opt-in behavior stays unchanged. |
| [OldOrigin::create/start_ready](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_p5.rs:49), [p0/p0_to_p6](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_early.rs:134) | create runs short context/profile CLIs, constructs Pair without children. start_ready uses Pair::start; canonical p0 uses Controller::start. Its first frozen children stop before Legacy capture: retain provisional identities and bind later. |
| [Controller::new/check/start/status/kill_wait/Drop](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_control.rs:172), [normal request/wait/reconcile](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_normal.rs:165) | Guard before create; check services both handles. Register before startup wait, bind verified decoder identity later. Sample before kill/shutdown request; close once after actual wait. Reconcile is too late for a live sample. Drop starts no query. |
| [relay owner](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_relay_owner.rs:81), [relay sample](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_relay_live.rs:95) | Preserve separate start/check/normal-finish/Drop ownership outcomes. Direct Node.status calls require the shared Stats tap, not only Controller::status. |

Sample PrivateBytes for **every original resident profile incarnation**, frozen/current and setup before verified source/nonce. Short account/enroll/context/profile-create/migrate/IPC/status/query children and relay require ownership/outcome/outer-cleanup accounting, not the profile384MiB RAM assertion. Profile-create CLI is not runtime-from-profile. Relay exclusion from this criterion is explicit. Extra original profiles/overlapping owner processes fail registration.

## Lifetime, data and attribution

One thread-affine ObserverGuard starts after A7 admission and before OldOrigin::create. It outlives profile/Pair fields but drops before the final capture guard, including constructor failure. Continuations retain the ledger and original deadline/clock; nothing restarts at P11.

Small API: register(&mut Child, owner/profile/image) -> Token; bind_verified; due(handle, token, phase); observe_stats; before_stop; close(actual outcome); finish_coverage. A thread-local record registry serves shared hooks, not replacement profile ownership. Distinct query children cannot recursively invoke profile sampling.

Token binds run/attempt, monotonic start ordinal, role, canonical profile/root, image and actual PID. Source/incarnation starts unverified; later bind the same original Legacy witness or decoder-verified PID/source/nonce without rewriting offsets. Reject duplicate/foreign binds, reused nonce, overlap, double close or cross-run merge. Only a new lifetime may reset Stats.

Memory query takes the concrete **&mut Child**, not arbitrary PID. Direct try_wait must show that original child alive before and after; returned PID/creation UTC ticks must match its first valid sample. Shell retains Process.Handle while reading StartTime/PrivateMemorySize64. Death/reuse race invalidates attribution, regardless of a new process with that PID. Application identity supplements this OS bracket.

Rows: monotonic query start/end, phase/reason/token/ticks/bytes/outcome. Summaries: sampled maximum, lifetime endpoints, first/last valid sample, max gap, boundary tails, missed slots/query cost/failures. Do not sum separate maxima into an alleged simultaneous peak.

## Cadence and closure

Sample after spawn and before kill/shutdown request. Boundary samples are additional to progress attempts, at most once per5 seconds per incarnation since start/previous progress attempt. Common alive/wait/check hooks service both owners, not per-message external polling. Deduplicate one opportunity; never relabel an older sample as a fresh stop sample.

Record synchronous/query gaps and missed slots; no backfill or hard5-second/unsampled-peak guarantee. Silently skipping an available due opportunity is a defect; disclosed synchronous gaps are sampled coverage. No arbitrary new gap cutoff or workload/timeout tuning.

| Event | Required result |
| --- | --- |
| Unexpected exit; missing RAM start/pre-stop; query/schema/identity/cap/deadline failure | Failed coverage retained, no zero/default/retry loop or further successful work. Cleanup proceeds. |
| Intentional kill | Fresh pre-kill query while live, then kill/wait even if observation fails; close once. Cleanup cannot turn failed coverage into PASS. |
| Normal shutdown | Pre-request sample is the guaranteed boundary. Due RAM sampling while waiting may race expected exit: record unsampled closing tail, not successful zero. Require valid prior boundary plus existing ack/exit0/wait/final/EOF. No post-shutdown IPC or query after wait. |
| Constructor/unwind/error cleanup | No new query in Drop/after deadline. Mark abort/missing boundary, retain rows; observing cannot delay known-child cleanup. Missing/unresolved outer receipt fails. |

A8 adds no blocking destructor and does not rewrite legacy profile wait semantics. Query cleanup reuses A7's bounded local owner/independent outer Job; whole profile cleanup remains a final-run gate.

Bookkeeping bound: existing Controller outcomes/operation streams<=32; conservative **36 lifetimes** covers those stops,2 live handles and2 Pair setup starts, not permission for extra restarts. Two exclusive owners over original2400 seconds give total live time<=4800 seconds: progress<=960 plus<=72 boundaries = **1032 query-attempt rows**, <=36 summaries. Failure rows count against attempts. Bounded phase enum/IDs; overflow fails, never trims/resets. Reconcile every token with its original profile and one terminal outcome.

## Stats source and one decision

[Stats7](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs:65) contains numeric max_native_bytes/max_commit_app_bytes, defaulted per process. [status3](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime7/runtime.rs:365) serializes owner.stats under top-level stats with version3/indexed/Ready/source identity. Native high water comes from raw.len(); commit high water from CommitReport.app_buffer_bound_bytes ([data2](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime7/data2.rs:216), owner.rs:845). These are serialization/app-buffer-bound reports, not RAM measurements.

Tap all successful A7 status3 calls, including direct Node/relay paths. A Value/PID alone does not establish lifetime: associate a candidate with the active profile/IPC token at query entry; accept only at the same owned-handle checkpoint, with unchanged lifetime/source and live bracket. At most one pending aggregate per owner; unvalidated candidates cannot close successfully. Capture after readiness, at existing progress status calls and before intended stop when IPC is available. Do not add IPC polling in held/shutdown paths. Missing all current Stats or skipping an available required snapshot fails; documented IPC-unavailable tails remain gaps, never zero. Missing/null/string/bool/foreign/regressing fields fail. Frozen children have no invented Stats7.

Assert sampled PrivateBytes<=384MiB and observed Stats max_native_bytes<=128KiB/max_commit_app_bytes<=192MiB. **Decision requested:** accept Stats as observed process high waters through the last valid live status, with offsets/end gaps, not terminal-inclusive maxima. Existing [verification::finish](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/src/ordered_dialog/runtime7/verification.rs:97) emits only counters/resolved; killed children may have no final record. A stopped repository cannot recover missing process Stats. Recommended resolution keeps A8 test-support-only and forbids an unqualified final/lifetime-maximum claim. If section6 requires terminal-inclusive Stats, root must first approve a different instrumentation/coverage premise; do not invent fields.

Retain [resource checks](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_pressure.rs:74): connections/peak2, mailbox/peak16, frame1MiB, bidi4/uni0, window64 per context. Cache64MiB, plan16MiB/headroom256MiB, app-buffer192MiB and backend epoch128MiB remain code/config bounds; BackendStats I/O/length is not resident cache occupancy.

## Bounded query and implementation scope

Reuse A7's private pipe/child owner, original sequence/create_new logs/effective deadline, cleanup reserve and verified outer containment. Add only a closed internal memory-query kind/parser: stdout/stderr<=4KiB; exact numeric PID/ticks/PrivateBytes JSON requires successful exit/wait/both EOF. Diagnostics contain safe metadata only. No spool/detached reader/general public executor; status routing/sanitization/original JSON stay unchanged. Proposed per-query work<=2 seconds intersected with original bounds; no extension of2350/2375/2400.

Use admitted absolute System32 WindowsPowerShell, closed direct-.NET command with numeric owned PID, no PATH spawn/Get-Process shadowing/user script. Future mandatory inventory binds shell path/hash/x64, command bytes, resolver/no-shadow checks before/after, owner/attempt/admission and every query outcome/empty Job. [Existing dependency seam](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s53-runtime-dependencies.ps1:18) checks x64, workspace/harness shadows and scalar Application resolution to System32; metadata-shell stays distinct. Read-only metadata now: powershell.exe495616B/SHA256 `60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f`, regular file, neither local shadow exists. This is not future resolver admission/execution proof.

After review: one memory module/tests, query helper only if needed; narrow root OwnedChild/spawn, Node/Pair, Controller/normal hooks, private A7 query reuse/Stats tap and deadline/auxiliary-outcome access. Reuse relay outcomes/ledger; no product/protocol/config/dependency/unsafe change. Prefer one increment; split query core/lifecycle hookup only at a genuine coherent boundary, not repetitive provenance rounds.

## Fixture premises and regression matrix

| Check | Premise / meaningful negative |
| --- | --- |
| Pure cadence/coverage | Inject monotonic clock/numeric results:5-second throttling vs boundaries, long/query gaps,1032/36 limits, failed-not-zero, retained restarts. |
| Pure attribution/lifecycle | Explicit synthetic identity records: reject reused PID/different ticks, foreign owner/nonce/profile, overlap, duplicate bind/close, missing setup. After-wait attempt must spawn zero queries. |
| Pure Stats | Real version3/Ready numeric schema; reject absent/type/regression/foreign/over-cap fields. Frozen fixture lacks Stats7; last-live is not terminal. |
| Minimal Windows | One known benign pinned sleeper under admitted Job; real start/progress/pre-stop query, positive bytes/matching ticks, then actual wait/empty-job receipt. No exact-byte constant or whole-messenger inference; no network/private state/grandchildren. |
| Failure/A7 regression | Pure malformed/overflow/exit-race/pending injection; directly affected A7 pipe/deadline/error selectors only. One closed stalled-query OS case only if A7's actual timeout proof cannot cover reuse. No unkillable process. |

Future verification: edition-correct format -> strict Clippy -> compile-only -> exact isolated stable runner; Jobs2/BelowNormal/offline/locked, finite per-command limits, fresh scratch/preserved failures. These fixtures are unexecuted premises. Root reviews the Stats decision/contract before implementation; no separate doc-only publication cycle. Stop here.
