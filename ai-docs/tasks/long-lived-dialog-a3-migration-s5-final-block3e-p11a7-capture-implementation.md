# P11A7: opt-in S54 status capture before durable output

2026-11-02. Baseline `4d40bb3f5e21fb5e9108c3a702ccf8a10077153b`, tree `d642ec4fc25cfb408b3865b7a6faad1cd886b5b0`. Original task SHA256 `2d1a0e4b19112be3a499948182fac4d7fbc97972e8ab3d411b9f1eca4b7fa845`; root gap decision `8568b5931988a9c21102731248c9e2e81bf277931fc7b881e568c774250d02de` verified before implementation. The earlier gap report/JSON and accepted root decision are included in this candidate, without a separate doc-only publication cycle.

Implemented only the dormant, explicitly admitted S54 status capture prerequisite. **No messenger CLI/runtime/profile/store/key fixture/network/enrollment/F/P/field or whole S54 was executed.** No memory observer, phase composition, final runner activation, Cargo/dependency/target/unsafe-policy changes, commit or push. Actual tests use only the pinned bundled PowerShell child with closed benign modes, as explicitly authorized by the root decision. Whole-run binding remains an activation gate.

## Exact hook and lifetime

Ten source/fixture paths are pinned with bytes/SHA256/normalized Git blobs in executor-review.json and the candidate manifest. Five existing support/harness paths change; four Rust modules and one closed PowerShell fixture are new. All changes are test support.

`json_cli` intercepts only structured `ordered status/status3 --ipc-file PATH` queries when the capture scope is active, before entering `cli` and its raw Output/error interpolation. Unsupported argument shapes for those exact status verbs fail closed. Non-status calls and all non-opt-in legacy/S53 queries take their unchanged path. This covers root direct frozen JSON status/old_status and Node::status, including current and frozen early/offline/kill/readiness calls. The guard starts before `OldOrigin::create` and its old_pair ancestry. It is moved into the last Controller field, so it remains alive through constructor failure cleanup, synchronous continuations, Controller::drop and preceding field destruction. It is thread-affine and rejects reentry; no whole selector was enabled.

The deadline helper adds only two capture accessors. A status reserves a monotonically increasing original command sequence, original run-directory create_new destinations and original absolute/effective local bounds. Entering/dropping capture does not recreate that managed scope, extend its clock, change its run directory or reset the counter. Capture reserves up to two seconds inside the existing whole bound; its work bound can only shorten the effective bound. Error cleanup has a finite local cap within that same whole bound, not a restarted absolute deadline.

## Admission and process ownership

Admission requires the managed deadline plus a rendezvous with the existing independently armed outer Job owner. The actual test wrapper enters the existing non-breakaway KILL_ON_JOB_CLOSE job before launching the stable runner; it verifies its launcher process HANDLE. Each actual Rust harness creates a bounded, attempt/owner/PID/nonce-bound join request and waits before capture spawn. The ancestor opens that exact live process, calls the existing VerifyChild membership check, checks MainModule path/current image SHA/start time, and atomically emits the bound permit. The raw receipt contains eight actual handle-membership records; JSON or an environment marker alone is not claimed as containment proof. All capture launches overwrite flags with NO_WINDOW/UNICODE_ENVIRONMENT and no breakaway.

Capture uses a separate std Child owner, never the legacy unbounded OwnedChild destructor. try_wait(Some(exit)) records actual observed exit and is followed by wait on its cached confirmed exit. Timeout/cap/read/error cleanup requests termination and polls only within the finite reserve. Constructor/unwind Drop applies the same bound and writes safe facts. An unconfirmed wait records cleanup_pending, poisons the scope permanently, and does not claim a dropped HANDLE terminated anything. The already armed outer job continues to own containment. The deterministic pure pending-state test proves that healthy/further successful work is rejected; it does not create an intentionally unkillable process.

The actual outer receipt records launcher return code0, work_expired=false, all eight admitted live harnesses, and empty job both before and after independent cleanup. It remained armed through receipt writes; Finish only follows the empty-job check. No local pending, watchdog termination, missing receipt or later outer cleanup can convert a failed functional capture into PASS. The future whole runner must establish equivalent binding before activating this Controller; it is not implemented here.

## Bounded transport and sanitize-before-write

The reader uses safe Tokio1.53.3 Windows NamedPipeServer APIs: two unique local-only, first-instance, inbound overlapped servers, synchronous child write-end Files, finite fair try_read chunks and a current-thread I/O driver advanced on every loop. Parent Command writer copies are closed immediately after spawn so EOF reflects real closure. No anonymous Tokio ChildStdio blocking adapter, reader thread/task, blocking join, synchronous peek or raw disk spool is used. Both streams are drained concurrently; stdout retention is capped at1MiB and stderr is counted/discarded, with its own1MiB ceiling. Counts may exceed the ceiling during bounded cleanup discard; they are actual observed counts, not retained buffer size. Success requires actual exit/wait and both EOFs before deadline.

Only a complete successful bounded UTF8 JSON value can be returned. The original Value is retained unchanged in memory for identity checks. A separate clone goes through the existing exact public_fingerprints function: object `keys` becomes `keys_fingerprint` = BLAKE3(serde_json::to_vec(keys)), then recursion continues through remaining object/array values. Only that serialized sanitized copy is written to the create_new stdout file; stderr is always opaque metadata. Raw byte buffers/chunks use Zeroizing. No claim is made about automatic wiping of serde Values, allocator copies or OS pipe buffers.

Invalid JSON/UTF8, nonzero exit, stream cap, deadline, read/spawn/sanitizer/write failures return bounded safe classification/known code/counts/parse location/ownership facts. New writers never format arbitrary stdout/stderr or a sanitizer error's payload. Error stdout is empty; no raw fallback or partial successful JSON. The legacy raw-error wrapper never receives opt-in failure Output. Pure tests inject sanitizer error text and a real read-only writer failure, test create_new collision preservation, original/nested fingerprints, exact verb routing, scope restoration, sequence0/1/2 and permanent pending failure. Existing deadline tests cover expiry/no-spawn and nested local minimum restoration.

## Actual checks and evidence

Immutable raw base: `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3e-p11a7-01/`. Before editing/overwriting, fifteen preimages were archived: five sources and ten compiler images. Failed Clippy01 source/helper revision (17 inputs) and its actual logs/receipt remain preserved. It rejected a non-mutable closure binding; the correction did not change the capture contract. The first check-wrapper revision is preserved separately: before compile/actual tests, only evidence accuracy was adjusted to initialize failure facts and anchor the admission wall-clock bound before Job construction. Validation/fixture inputs were frozen before the first synthetic run. A documentation audit initially expected a nonexistent byte-count property in the existing compiler source inventory; that generator/failure is retained, and SHA checks were preserved. No test rerun followed prose/audit-only edits.

| Final check | Actual child exit | Seconds |
| --- | --- | --- |
| fmt-02 | 0 | 4.236 |
| clippy-02 | 0 | 12.023 |
| compile-01 | 0 | 101.467 |
| pure-01 | 0 | 42.458 |

Format -> strict Clippy -> stable compile-only -> focused checks completed in that order. Jobs2/BelowNormal/offline/locked; x64 verification-scale. Compile watchdog1190/1200 seconds, Clippy590/600, format/tests230/240. Rust harnesses ran only through run-cargo-tests-stable.ps1 at fixed paths. All13 exact selectors passed, each isolated, with0 failed/ignored; six aggregate invocations selected0. There was no broad accepted-stage replay. Existing fixed artifact counts6/9 and Cargo graph remained unchanged.

| Synthetic mode | Actual child PID | Classification | Seconds | Observed stdout / stderr bytes |
| --- | --- | --- | --- | --- |
| valid | 56588 | success | 1.213 | 95 / 19 |
| pressure | 61804 | success | 2.842 | 524396 / 524307 |
| malformed | 59140 | invalid_json | 0.887 | 29 / 19 |
| invalid-utf8 | 33144 | invalid_utf8 | 0.934 | 3 / 19 |
| nonzero | 65928 | nonzero_exit | 0.932 | 95 / 19 |
| stdout-overflow | 60040 | stdout_cap | 5.026 | 1122304 / 19 |
| stderr-overflow | 9600 | stderr_cap | 4.995 | 0 / 1114131 |
| stall | 12476 | deadline | 8.060 | 0 / 19 |

All eight actual children have confirmed exit/wait, both EOFs and cleanup_pending=false. Pressure uses concurrent child writes on both streams (524396/524307 bytes); successful original nested identity fields remain available in memory while durable output contains fingerprints. Malformed/truncated JSON, invalid UTF8, nonzero17, per-stream overflow and stall return errors with no stdout/stderr sentinel in command logs or error strings. Original public identity sentinel fields/values are absent from all durable command logs. Shell bytes/path are SHA256-pinned to `362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139`; source bytes equal the compiled immutable fixture. No grandchildren/network/profile operations are present in the closed fixture.

Compile and focused-run input inventories match before/after; all501 final units are fresh and the independent frozen guard verifies. All590 currently enumerated compiler source inputs were rehashed; the included PowerShell fixture/job/helper/shell pins are additionally checked separately because the compiler source enumeration does not include arbitrary ps1 files. Nine of ten preserved compiler images and all four field images remain unchanged. Normal messenger CLI/field/state executables were not executed. The only changed fixed image is migration_s5_process: 17976320B/SHA256 `ae6a10b65a014a8f7c8fbe9a4ffdbc63fb81165b648ff2a7d19563fae8875163`.

The p11a7-evidence directory archives real receipts/logs, source inventory, review, immutable validation helper copies, original wrapper revision, membership/admission bindings and all case facts/raw-file pins. The complete raw compile/run captures, expected manifest/inventory, actual command files, preimage EXEs and failed source revisions remain at their pinned raw paths. No raw historic evidence was rewritten. Log-only EOF blank lines remain explicit evidence exceptions, not source fixes. Candidate manifest is generated after all documentation and archive files, with normalized Git blob hashes; documentation generators are excluded from historical validation-input pins.

For reproduction use a fresh ignored base, update the copied helper base/immutable capture paths, compile without Run, wait for its actual capture, freeze the expected manifest and helper/fixture pins, then run the exact isolated manifest with aggregate skips under check.ps1's verified Job/rendezvous. Preserve new receipts and do not reuse prior step files. A changed bundled shell must be explicitly re-pinned before it can run. Historical review.py audits this exact baseline/raw attempt, rather than pretending the raw evidence is embedded in the archive.

## Stop boundary

Actual Windows capture/EOF/deadline/cleanup and sanitizer core ran on benign synthetic children. Actual frozen/current messenger status calls, early S54 construction and failure integration are source-reviewed/compiled only. Pending termination failure is pure state injection, not an OS-failure experiment. Whole S54 admission/finalizer/memory/phase composition remain gated. Unrelated navigation3/root work and the empty index are preserved. Submit this exact candidate for root review; no commit/push until explicit acceptance.
