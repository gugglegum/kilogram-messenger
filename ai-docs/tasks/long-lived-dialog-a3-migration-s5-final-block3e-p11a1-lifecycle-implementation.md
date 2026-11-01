# P11A1: dormant normal lifecycle and exactly-once final reconciliation

2026-11-02. Implemented on `f70aec67a304354e6130198d48020b448951376e`, tree `6d50a4800df02043ad52a83a2a5a4d6db1608350`. Task6420B/SHA256 `145737a445110ecd067e3cc27bd59489905df63d6be21947455c043f5c38a9ab`; accepted P11 contract24568B/`5d634a445e9998a2cf2e5eeeaaff88633a9ac5b5d4dca802001a404401d7daa0` and narrower root grouping remain unchanged.

This increment supplies reusable normal-stop/final/summary support on the existing Controller and relay. **No actual owner/relay startup/shutdown, scenario, state/private fixture read, enrollment/F/P/field action, P11 composition or commit/push occurred.** Runtime joins, EOF, persistence, process cleanup and timing remain unmeasured assertions. No phase advancement, whole accounting constants, native completion marker or whole PASS was added.

## Exact source changes

Nine paths/raw114314B under `apps/kilogram-cli/tests/support/`; exact raw SHA256 and normalized Git blobs are in `.tmp/s54-block3e-p11a1-01/review.json` and the durable evidence executor-review.json.

| File | Bytes | Change |
| --- | --- | --- |
| migration_s54_control.rs |19701| Same slots retain optional normal state; check distinguishes expected exit; killed/reopen/normal guards and retained relay outcome. |
| migration_s54_normal.rs |10150| Dormant request_normal/wait_normal_both/reconcile_normal_both and guarded finish_normal_relay. |
| migration_s54_normal_tests.rs |3519| Three pure original-key/deadline/ack/wait/either-order/reopen predicate tests. |
| migration_s54_observations.rs |16577| Preserve actual final record separately after the existing single Operations.finish. |
| migration_s54_normal_observations.rs |3650| Normal identity and read-only final/EOF/closed-ledger/source/root-order validation. |
| migration_s54_normal_decoder_tests.rs |6986| Four actual Decoder/Operations synthetic-record tests. |
| migration_s54_oracle.rs |21095| Read-only closed-stream validation and checked complete summary with actual counts/identities. Existing complete/finish/reconcile bodies unchanged. |
| migration_s54_relay_owner.rs |9416| Normal published stop/exit0/wait finalizer, requested-exit checks, explicit forced-cleanup evidence and one pure predicate test. |
| migration_s54_source.rs |23220| Same bounded end read, with a separate normal entry checking actual repo source/profile binding. Existing end caller behavior preserved. |

No product runtime/crypto/state/protocol change or general process framework. New implementation is nested in existing Controller/Decoder modules; no P11 selector or two-cut driver. Other tracked changes are the preserved navigation3; historical untracked files and root Current handoff were not edited. Index remains empty.

## Ownership, requests and real waits

Each existing Running slot holds optional Stop metadata: actual source/PID/nonce, original min15s/whole bound, acknowledged and waited state. request_normal preflights current indexed handle/decoder identity against the original source, checks actual liveness, then stores the unresolved state **before** invoking real `ordered shutdown --ipc-file`. It requires the actual successful CLI output to be exactly the shutdown-accepted marker. Acknowledgement alone cannot close or reconcile a stream.

Controller.check preserves the whole deadline, polls decoder logs and requires unrequested children and the original relay alive. Only a specifically requested normal child may have actual exit code0. Its identity is checked using a separate retained-key getter that remains available after final; the existing live-only incarnation getter still rejects closed streams. Thus a first acknowledged owner can finish and emit final before the second shutdown request without being misclassified as an unexpected exit.

wait_normal_both requires both original acknowledged requests, retains each known handle until actual wait, and detects either exit order. Each actual wait must be code0 and within its original request bound; no whole/phase reset. Waited handles move into the existing pending_stops slots with actual normal-stop/outcome metadata. Missing ACK/final, nonzero exit, deadline or source failure leaves owned/unresolved state, so start/reopen remains forbidden. The pending counts field keeps its previous actual live sample for killed callers; normal reconciliation explicitly ignores it and uses the retained final record.

ready_to_read checks both retained normal waits and absence of both running owners before either private source open. reconcile_normal_both polls final tails, validates both records/EOF and actual bound stopped source ends, then publishes both resolved outcomes and final counters only if both validations pass. A failure retains both pending records. Normal records cannot enter the earlier killed reconciliation path; a normal-stopping child cannot be accepted through kill_wait. Existing killed close/reconcile semantics remain intact.

Controller Drop still cleans the same known handles without deadline checks, including running normal-stopping slots; waited pending handles remain owned and are marked unresolved on failure. Drop outcome facts include normal-request/ACK/wait state where available. A fallback kill/wait is cleanup evidence only and cannot satisfy normal acceptance. No process-name kill or detached handle was introduced.

## Actual final evidence, without double finish

The existing Decoder final branch calls Operations.finish once and now retains its actual Counts/key/sequence. The last native commit counts and PacketObservation remain separate. A legitimate final attempts/aborts adjustment therefore survives even when the last successful native commit had smaller attempts. Normal source validation never replaces these values with a last live Stats sample.

normal_final requires a real final record, matching original key/sequence, final_seen, complete newline fragment and actual post-wait file offset==EOF, plus an already-closed ledger stream with exactly those final counts. Duplicate/unresolved/gapped/regressing/post-final observation records are rejected by the existing Decoder/Operations checks; missing final or a truncated/unconsumed tail cannot be guessed. The normal validator is read-only: no second finish/reconcile_final and no stopped-source synthesis of a missing native commit.

source::normal_end opens only the original stopped owned directory and checks actual source_id and profile account/device against the requested incarnation/Node. It delegates to the same bounded frontier/generation/last_commit read as source::end; the latter's old path remains unchanged. The observed stopped Text point must equal start_sources plus actual final encrypt/decrypt, using checked addition. Last packet sequence must match final sequence; stopped root generation cannot precede that packet, and equal generation requires equal operation ID. A later root/operation is permitted for legitimate receipt/control/locator commits. Zero-work streams require no packet record or fabricated success. Closure facts retain actual root/end/EOF/final counters and last native context/op/generation/sequence/deltas for later accounting.

Operations.closed validates a known closed resolved stream read-only. Operations.summary requires all bounded streams closed/resolved and current constructors zero, then checked-adds actual encrypt/decrypt/attempts/abort/ambiguity/constructor fields and returns actual incarnation/sequence/count rows, owner+operation IDs and observed owner generation entries. It does not install totals, infer overlap, advance phases or replace the existing complete wrapper. Tests retain an actual final abort1/attempt2 adjustment after decrypt1/attempt1 commit, while unchanged success counts remain tied to native commits; later full P11 can reject nonzero aborts using the preserved observation.

## Normal relay closure

finish_normal_relay first requires no running/unreconciled owners and a closed ledger summary. It acquires exclusive locks on both original state directories, rejecting an accidentally retained repository reader, and holds those locks through relay completion. Only the existing original relay is used. Its finalizer preflights liveness/owned URL, stores requested state, creates and syncs its own stop file once, then waits actual exit0 within min15s/original deadline. Controller/relay checks distinguish requested shutdown from unexpected exit.

Actual PID/URL/stop-published/wait/code outcome is stored in Controller.relay_outcome **before** releasing the relay. Further start/replacement after final relay completion is forbidden. The existing Drop fallback still kills/waits the known relay when needed and writes separate best-effort cleanup evidence; it now records whether a forced kill was actually attempted. Requested-but-unresolved/forced cleanup cannot become normal acceptance. A stop file alone and pure tests do not prove that the real relay process exited gracefully. The fixed relay image/lifetime are unchanged and were not executed.

## Actual verification and immutable evidence

Raw evidence base `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3e-p11a1-01/`. Seven accepted EXEs and four initially changed source preimages were archived before edits; the untouched source.rs preimage was added before its narrow binding change. Both original and extended manifests remain; all12 preimages hash-verified. Intermediate pre-clarification sources for Clippy01/02 are preserved. No failed Clippy, compile or test run occurred; no receipt was overwritten.

| Actual check | Exit | Seconds |
| --- | --- | --- |
| Final CLI/all-target strict Clippy03 |0|10.3387856|
| Stable locked/offline compile-only01 |0|88.7883238|
| Targeted stable pure01 |0|16.5582989|
| Format01 |0|5.248237|
| Exact source/binary/input/index review |0|9 source pins/571 current compiler inputs|

Clippy01/02 passed11.7777326/10.2294172s during source review before two refinements: retain the normally finalized identity without weakening the old live getter, record forced relay cleanup explicitly, and bind the stopped source to the actual original source/profile. Final Clippy03 checked that final source; one compile revision and one targeted run followed with sources frozen. An attempted edit with stale formatting context was refused without modifying files, then applied against current text. The first evidence-packaging pass failed on an outdated report filename; packaging-failure.json retains that failure, then the correct lifecycle report path was bound. Neither incident required a source/check rerun.

Native watchdogs: Clippy590/600, compile1190/1200, pure/format230/240 seconds; Jobs2/BelowNormal, verification-scale/x64, locked/offline, one stable path lane. No hash-named harness execution or broad unchanged G3/P7/P8/P9 replay. Targeted run has20 passed/0 failed/0 ignored:8 new plus12 precisely affected decoder/ledger/source-kill/relay/deadline-bound predicates. The unchanged generic deadline suite was not rerun. Other harnesses selected zero tests. Exact names/arguments are in test-selection.json/pure.stdout.log.

Coverage: original source/PID/nonce/minimum deadline; expected versus unexpected/nonzero exit; strict actual ACK and ACK-without-wait; both exit orders/both-wait source gate/pending-stop restart exclusion; actual final adjustment and last packet retention; read-only repeated validation versus rejected double finish; missing/duplicate/unresolved/regressing/gapped/post-final records; unconsumed/truncated EOF/foreign key; stopped source mismatch/extra success/stale generation/conflicting same-generation op versus legitimate later non-native root; actual zero-work closed/reopened stream summary; published relay stop/exit0 versus forced cleanup. All new cases are synthetic public metadata/observation predicates used by the implementation, with no OS process fixture. The affected existing incremental log test uses its synthetic test file only, not messenger state.

Compile before/after and pure before/after inputs are identical and current:571 source hashes,501 units, all501 fresh during pure; independent frozen guard true. Normal non-test CLI stays50473984B/SHA256 `f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`. Migration stable harness17125376B/`93a7720acae26c788b765eefee3cd1afbc87c4da2440828753fa72dbf6eae93b`; other five stable EXEs and four field images unchanged/unexecuted. Exact review23763B/SHA256 `98560bc814d0ef9ec98a14852d4f12f247b78df762653cc73805fbd7c0217f45`.

Durable `long-lived-dialog-a3-migration-s5-final-block3e-p11a1-evidence/` carries receipts/logs, source-input/provenance/review pins and bounded reproduction helpers. Full raw captures/independent expected manifest/preimage EXEs/source revisions remain at the immutable raw base. To reproduce, use a fresh directory/update helper paths, compile without Run, derive an independent expected manifest from that compile capture, then execute only the selected stable pure tests. Original review.py audits this historical attempt and immutable step IDs.

## Remaining boundary

Actual IPC acknowledgement, normal exit/join/wait/EOF, stopped source binding and relay/file/lock/fallback behavior are runtime assertions, not established by pure predicates. Full terminal source/receipt/Native reader, same-route comparator, two-cut/restart/final composition, memory observer, sanitize-before-log capture and runner inventory/admission/whole-result gates remain **unimplemented mandatory subsequent slices**. No workaround or partial whole result is introduced. Ready for root review; stop before publication/next slice.
