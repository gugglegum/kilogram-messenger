# hint-offline-01: adapter failure before offline delivery

2026-11-20. One accepted physical PC/laptop attempt was executed after exact4 publication. It **failed in the test adapter before any application send or recipient restart**. This is not an offline-delivery pass or evidence of lost messages. No second attempt or product correction followed.

Publication: `9a8b280c1267a8a999b5b2a7e0d023ecbed8d640`, parent `018a62f8935798574eee8203374b06be1b3d5958`, exact4 accepted paths/blobs, index empty, push and live origin verified. Allowlist SHA256 `6c780f2cd3b7351d37c2994020e34a79a942b2b6f8f8cde4b05747647ba16390`. Proof `.tmp/terminal-t1-3e/offline-recipient-publication-01/publication-final.json` SHA256 `567f88dce56c698b2872b9044b46e5cf4ae7c49ac3b613b4d23b98530b42c159`. Foreign8 and other notes were preserved. The initial auto-review rejection occurred before execution; the human then explicitly authorized this and future master commit/push. The unchanged exact publication command succeeded. Both events remain recorded.

Runtime acceptance was materialized under the conditional [root authorization](terminal-t1-3e-offline-recipient-root-acceptance.md), changing only the two approval booleans and verified commit. Its SHA256 is `9733a21e71f14587758220dc436c512c3ee5ba590cfde771c89965e79b88ce71`; the binding proof is in the publication directory. Reviewed plan/wrapper/inventory and existing A2 binaries were unchanged. No Rust build/test, clock/firewall/network/profile/task/agent installation changes were performed.

Fresh preflight confirmed exact A44024/start134395772085441302/session261956141a2e4d5ca0aa8382f882bad2 and B11884/start134395656927981748/sessiona6e8faadcf4d4979b422f85578ae7c64, native/raw ordinary token views, idle/readiness/original-six and expected network/port availability. The supplemental B CIM process query returned access denied; its failure is preserved. A read-only `System.Diagnostics.Process.GetProcesses` query then confirmed no conflicting Kilogram process without changing rights. Evidence `.tmp/terminal-t1-3e/offline-recipient-preflight-01`, including `resolution.json`.

## Actual result and cause

Controller exit1, no timeout,453.766s. Application elapsed331.000s. Fresh Setup completed on both hosts. The original90s admission gate passed: four reciprocal leases observed at60.532s, decision63.938s, no failure-only diagnostic window. A role0/B role1; Connect15.250/42.032s. No policy/discovery rejection occurred. A had2 initial outbound admission errors and B2 listener prerequisite rejections before admission; actual TLS errors0. Their inner predicate remains unattributed and is not the observed adapter failure.

Primary error: `RuntimeError: A: final history contexts differ from deliveries`. Call chain:

- `scripts/test-terminal-t1-3e-offline-recipient.py:108` calls inherited `histories('initial', ...)` with two admitted contexts and zero messages.
- `scripts/test-terminal-t1-3e-driver.py:450` collects both contexts and calls `verify_final_history(snapshots, self.messages)`.
- `scripts/test-terminal-t1-1f-driver.py:294` derives expected contexts from delivered messages, yielding an empty set, and rejects the two context entries before checking their rows.

The inherited verifier therefore rejects even two valid empty initial histories. `.tmp/terminal-t1-3e/offline-recipient-result-01/reproduce.py` invokes that actual verifier locally with two empty contexts and no messages and reproduces the exact error, with no network or product process. Prior functional helper checks mocked `histories` at `scripts/test-terminal-t1-3e-offline-recipient-check.py:119`, so they did not exercise this inherited precondition. The actual field history rows are not reconstructed from this synthetic reproduction; the failure's context-count condition itself is deterministic regardless of row content.

No sends, pending messages, recipient stops/restarts, reconnect gates or replies occurred. Both clients issued2 history requests then quit. Notifications observed0; notification coverage **not evaluated**, required/not-covered counts undefined. Delivery/history verdict false because the intended scenario was not executed. `offline-recipient-result.json` correctly records messages0/restarts0/passedfalse; do not relabel this as a product or notification failure.

## Cleanup and next boundary

Both Setup and both Chat owners have child exit0, acknowledged quit, EOF, empty process membership and no descriptor/forced/work-expired/error. Each Chat has14 validated typed records, complete startup/final coverage, worker saved/reaped/graceful exit0, connection ack/EOF/exit0 and runtime IPC/network/owner joins successful. Both native attempts were cancelled after the primary assertion (`state=failed`, `LAB_CANCELLED`) but their Job cleanup is confirmed empty, unforced and error-free. This native failed state is preserved, not reported as a successful attempt.

The suite restored and hash-verified the known original six files on both hosts. Final passive native/raw/process checks also passed: same exact agents, ordinary token views, idle/unblocked/readiness, originals and no Kilogram processes. Final native reads A01:19:48Z/B01:19:59Z. Evidence `.tmp/terminal-t1-3e/offline-recipient-result-01/final-state-01`.

Frozen failed run: `.tmp/terminal-t1-3e/offline-recipient-live-01/hint-offline-01`; controller receipts remain in prep02/execution01. Assertion-based evidence analysis is `.tmp/terminal-t1-3e/offline-recipient-result-01/analyze.py`; `public-facts.json` SHA256 `a814d4977e7d9ac347ef5426acc73f955a2848ede67987eee766918c5d066f54`. The earlier human report of a laptop window closing/PC exit1 predates this attempt and remains unattributed; this new controller exit1 is now precisely attributed to the adapter assertion, while its applications exited0.

Proposed next review scope only: give the initial snapshot an explicit two-context/empty-row assertion, retain strict final delivery-history verification, add a regression exercising the real inherited collection/verifier path rather than mocking it, then repackage affected pins for review before any fresh attempt. No implementation, new publication or retry is authorized by this result note. Result note is submitted for architect review, uncommitted; executor returns once with publication/result/cleanup proof and idles.
