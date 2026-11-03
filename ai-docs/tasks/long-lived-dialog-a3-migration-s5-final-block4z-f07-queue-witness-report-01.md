# Block4Z: purpose-bound original queued-request witness

2026-11-03. Baseline `e463ee55f0a113d775498aaa6c5c4179334d0f12`. The F07 failure is a shared test-witness contract mismatch: `Legacy::capture` accepts only original A17/A18 queued requests, while F intentionally queues A3/A4. `Controller::migrate` captures that witness before migration preview. The minimal correction derives a closed purpose from healthy existing capture admission and uses **F=[3,4],P=[17,18]**. All source/queue/envelope/AAD/body/identity guarantees remain. No product/storage/protocol/crypto code, workload, timer or attempt policy changes.

Before: actual P17/18 helper control passed; actual F3/4 helper failed with the same static error as F07. After:7 exact focused native tests passed, including both purpose sets,34 queue negatives,existing owner-scope positives/negatives and absent/stale admission refusal before source access. No production scenario ran. This is a source-review candidate, not a successful F runtime or whole S5.4 result.

## Public/static diagnosis and shared-call audit

The original source-defined `Error: exact original queued17/18 request` plus LF is42 bytes and SHA256 `5ddfe126df495cab8c7bb43447765fa46168768c7863e67ec992a7fa02b004ae`, exactly the retained public F07 stderr-prefix metadata. The proof uses only this known static source literal and public outer-result; no arbitrary error/hash recovery, raw command/runtime log reread, profile/key/DB access or historical store decryption was performed.

F source flow is initial empty-queue Legacy capture → real restart/two-each → accepted V offline close/wait/reopen and strict Backoff/frontier gate → A3/A4 queue and stop → migrate0 → shared Legacy capture → error before preview. Initial empty queue never enters the hardcoded lookup. Root-provided public F07 memory facts (five legacy/no current lifetimes,all explicit Kill/wait,23 successful queries) are consistent with that downstream location; they do not establish full V/F workload or migration success. F07 remains consumed.

Reviewed all shared capture/migration witness paths and nearby fixed-P assumptions before implementation:

| Witness/path | Applicability and result |
| --- | --- |
| `source::Legacy::capture` | F initial capture; P p0/p1 captures; shared Controller::migrate. Only demonstrated shared ordinal mismatch is queued17/18 lookup. |
| `Controller::migrate` in early.rs | Used by F and P. Capture precedes preview; same_identity,after_migration,accounting and committed endpoints use actual witnesses. |
| capture event/projection loop | Actual per-lane frontier bound1..=25,C2zeroText; suitable for both early sources. Request/body derives actual context/author/ordinal,not a fixed P count. |
| `Legacy::after_migration` / accounting::migration | Compare exact original sealed queue/spec/head/native/events and actual committed prefix membership/cardinality; no fixed17/18 assumption. Unchanged. |
| `late_old` B16..24 / `late_accounting` | Called from P transition stages,not F. Retained as P-specific assertions. |
| `current_old_encrypts` A17..24 / R1 A24 / early source_row format boundary25 | P-only transition/early call sites; F uses its own source/history mapper. Not generalized. |
| body constructor | Both controllers send oracle::Request::body(),including the existing P5 text label. Matching uses that exact original constructor; no body-label workaround. |
| P1 passive30s Backoff | Concrete remaining WholeP review risk, described below; not changed in this task. |

No additional shared fixed-P mismatch was demonstrated within this contract. The audit is static/source-backed and does not claim remaining F/P stages are runtime-validated or free of other defects.

## Minimal implementation and preserved checks

Only three existing support files change: `migration_s54_source.rs`, `migration_s54_source_terminal_tests.rs`, `migration_s54_source_scope_tests.rs` (174 insertions/17 deletions,mostly focused regressions).

The existing guard now returns the verified Purpose internally. `owned()` retains its Result<()> interface; Legacy capture uses owned_purpose() to obtain the enum once. This first calls existing healthy s54_capture::admitted_attempt(),then the existing exact current F/P attempt resolver and canonical origin guard. Unknown/stale identity is rejected before workspace/state canonicalization; absent/failed capture is rejected before source opening. The closed origin-guard body is byte-equal except returning the already resolved enum. Purpose is never selected from queue content,path substring,a caller-provided numeric range or an ordinal union.

Small internal QueuedRequests is the actual shared capture matcher. It chooses exactly [3,4] for FeasibilityF and [17,18] for WholeP,requires context index0/owner role0,canonical32-character lowercase hexadecimal ID and exact request_id equality. It retains an at-most-two bound and uniqueness set; original queue map cardinality and duplicate insertion checks remain as additional guards. Empty initial queues are still accepted. Later F/P exact cardinality/queued-frontier assertions remain unchanged.

The matched canonical ID and exact oracle Request feed the existing sealed-envelope decoder,DeviceEncryptionIdentity::open,encapsulation/ciphertext checks,AAD(version/context/request/lane),plaintext version/context/request/body comparisons and original sealed-byte insertion. Those blocks are not rewritten. Source event association,projection envelope/body binding,secrets/zeroization,source identity,all after_migration and terminal source/native/receipt checks remain unchanged.

`cause-and-scope-proof.json` reconstructs the complete baseline source by reversing only owned-purpose plumbing and queue routing. It matches the original file exactly. The before→after correction is only the closed match-arm split plus its static error label. The entire source tail from after_migration onward is equal. All518 other source files are pinned unchanged,including V scenario/offline helper,early/transition/accounting workloads,admission,compiler,historical probe,purpose,deadlines and product crypto/storage. Fresh empty scope fixtures use the owned Z administration; no profile/store is constructed by focused tests.

## Actual focused evidence

| Step | Original tool returns | Child/owner exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| format-01 | `4e039b → 4e039b` | 0/0 | 1.7576235 | 570/600/30 |
| compile-01 | `ad481d → 422c85` | 0/0 | 39.3799267 | 1170/1200/30 |
| rust-01 | `cd44c5 → cd44c5` | 1/1 | 4.1425425 | 570/600/30 |
| format-02 | `940fcc → 940fcc` | 0/0 | 1.8157644 | 570/600/30 |
| compile-02 | `2317f3 → 494428` | 0/0 | 38.6893785 | 1170/1200/30 |
| rust-02 | `0dfedc → 671b2d` | 0/0 | 8.7106265 | 570/600/30 |
| seal-01 | `ace90c → ace90c` | 0/0 | 6.3789204 | 570/600/30 |

Both compile steps are compile-only through scripts/run-cargo-tests-stable.ps1,profileverification,offline/locked Jobs2/BelowNormal. Cargo argv contains --no-run and --target-dir pointing only to target/s54-diagnostics-source-checks. Selected execution uses its fixed stable migration harness. No production image/origin/dep-info was rebuilt,replaced or rebound. Existing unrelated compiler/transcript/HEAD/session-key/offline matrices were not executed.

Before selectors run P positive first,then F positive against the old fixed-P decision through the extracted production matcher: P passes,F fails native101 with exact static queued17/18 error. This intentional failed check is preserved,not a runtime retry. After selectors run7 exact tests:

- F3/4 and P17/18 positives through the actual helper,including empty initial queue and returned canonical ID/Request equality;
-34 negatives across both purposes:cross-purpose/unrecognized ordinals,wrong context/role,wrong encoded request scope,malformed/noncanonical IDs,duplicates and third entries;
- existing exact P/F owner-origin controls and cross-attempt/namespace/origin/escape/missing-path negatives;
- no-active capture refusal through actual Legacy::capture before opening missing storage;
-7 stale/unknown identities rejected before even workspace canonicalization.

Actual markers/totals and seven one-pass native summaries are required by the seal. Tests call the same matcher used by Legacy capture; no parallel matcher is reimplemented in a fixture. Positive body expectation is the existing constructor; sealed-body preservation is structural here,not a new historical decryption claim.

Every receipt has verified actual handle/Job membership,waited child,bounded stdout/stderr EOF,no reader failure/work expiry,verified cleanup and after[]. Compile01/02 terminated owned residual PID66020/43408 respectively; executable identities were not captured and are not inferred. Other cleanup was nonforced. Scoped rustfmt/whitespace pass; successful native process inventory has0 relevant live images.

## Remaining risk and review boundary

WholeP P1 still waits passively30s for ConnectionBackoff after stopping B (`migration_s54_early.rs`). The pinned legacy idle Resume30s+RPC15s premise reviewed in V is relevant; WholeP must review this risk before eventual runtime. P's source cardinality,phase ordering and timing must be considered there. This task deliberately does not copy F's reopen into P,relax Backoff,increase timeouts or alter P workload.

Post-fix runtime migration,offline queue recovery and all later F hooks remain unvalidated. F07 remains failed/consumed; no F08 policy transition or production allocation,Prepare,activation,Run,WholeP,commit/push occurred. No new security/product semantics were introduced.

Final seal verifies1257 protected public/history/report/admin/image pins and518 other sources; all F01–F07 artifacts/preimages/images/origins,root navigation and unrelated changes are preserved. Public/static evidence only,no raw/private access. Finalization compares outside-allowlist Git status with baseline.

Source publication candidate:3 support files + this report/pins + unchanged X preparation and Y failed-runtime reports/pins =9 regular100644 paths,with raw hashes and normalized Git blobs. Large original evidence is referenced,not copied. Current source differs from historical F07 images/inventory; no historical artifact is restamped. Stop after one callback for root review. F feasibility and whole S5.4 remain unaccepted.
