# CS3.2b implementation review 01

2026-11-11. Baseline9507d74c92acd0e41705814b6d31bfee6ee8307a. Reviewed final candidate-gui.json SHA256eb510cdb353656067d575f734e3dcde078fd9a6d11d7d2c085c5d93aaa2aabcd,21 paths/raw1056559. Rootac4ea6 independently verified204 unique raw pins, normalized Git blobs, HEAD/index and clean check12/test06/compat01/test08/ui06-09/observe01-02 receipts. Evidence .tmp/clean-start-cs3-2b/root-review-verification-01.json. Actual executor commandExecution history and GUI action sequence checked; root did not rerun tests or claim to perform the dual GUI walkthrough.

Reviewed full new connection/locator adapter, shared profile/typed client/status DTOs, CLI/config/runtime diff, view/model/consent integration and focused tests. Same-descriptor identity binding, current lease+request-start freshness, exact offline two-slot recheck, independent signed locator exchange, immutable restart pins and ordinary graceful lifecycle are supported by implementation and evidence. The completed dual GUI/restart observations are valid evidence for this candidate; previous failed attempts remain failed. No transport/crypto redesign is requested.

Decision: changes requested before acceptance/publication. Three scoped findings below. Complete these together and return a revised ready candidate; no additional contract round.

## R1: Keep ownership after every post-spawn failure until exit is observed

Priority P1. In fresh_onboarding/connection.rs around467, startup error discards session.stop() and returns Err. Session::drop around676 only performs a finite best-effort kill/wait and discards an unconfirmed wait or error. Work::start around815 then sends Setup back with that Err. Unlike run_worker ordinary-stop monitor around772, this path can release the Setup lock/allow offline UI actions while the spawned process is not confirmed reaped. Early drain initialization failures after spawn and worker error unwinding have the same ownership boundary and should use the same narrowly scoped cleanup mechanism. This is a code-path finding, not a claim that the successful recorded tests left a process running.

Preserve the held Child and Setup ownership on startup/worker failure until actual exit is observed; expose unconfirmed cleanup as blocked lifecycle state. Do not return an ordinary completed worker result, allow another Start/offline action, or claim graceful shutdown merely because a cleanup timeout elapsed. Keep bounded individual stop/kill waits and no foreign PID control. Factor the small common owner/finalization path as needed, without a general process framework. Do not move blocking cleanup into rendering. Preserve the original start failure and forced/unconfirmed classification after eventual reap.

Add a deterministic focused test that models an unconfirmed cleanup result followed by observed exit: Setup/done must not be returned and offline actions must remain blocked before reap; after observed exit the failed outcome and same Setup may return. Cover the startup failure path, not only the normal run_worker stop path. An OS access-denied kill need not be forced in reality; a narrow fault hook/controlled result is sufficient. Keep a real ordinary failed-start/cancel and successful stop regression to ensure normal ownership works.

## R2: Clear obsolete waiting text after a current connected observation

Priority P2. run_worker around722 retains last_notice="Both locator imports acknowledged; waiting for current authenticated status." and prepends it indefinitely, including to current Connected status. This was observed in the completed GUI walkthrough. Use an accurate historical acknowledgement without a stale waiting assertion, or derive waiting from the current view. Ensure later disconnect/stale states do not retain an unqualified current-success sentence. This is a small user-facing correction; it does not invalidate the earlier lease/identity evidence.

## R3: Preserve existing CLI runtime error diagnostics

Priority P2. ordered2_cli::call previously propagated RuntimeIpcResponse::Error.message. The shared ordered2::decode_reply now maps every non-OrderedDialog variant, including a legitimate runtime refusal, to "unexpected runtime response". Consequently normal policy/validation/request-conflict errors lose their explanation even though the wire response is valid. The approved codec tightening required matching outer kind/version/opcode, not removal of existing CLI diagnostics.

Retain baseline CLI error reporting for the Error variant while continuing strict rejection of genuinely wrong response kinds and envelopes. Keep desktop-facing errors sanitized; this can be a narrow CLI adapter match rather than exposing raw messages in the new UI. Add a focused regression for a legitimate Error response versus wrong-kind/version/opcode. No unrelated CLI refactor.

## Verification and delivery

Perform targeted tests for these fixes and affected strict checks. Reuse prior broad cryptographic/dual connection/restart evidence where code paths are unchanged; do not rerun historical campaigns or unchanged compatibility matrices without cause. Verify corrected UI wording with a brief relevant actual UI observation on the resulting fixed image; do not present the old executable as the new image's GUI verification. Executor may choose a short dual connection smoke if needed to cover the changed ordinary lifecycle path. Record which evidence is reused and which new runs cover the changed paths.

Update result/recipe/pins/exact candidate, including this review as appropriate. Prior candidate/evidence/failed attempts retained; no unrelated BG/navigation changes. Idle/Jobs1, stable Windows harness/fixed EXEs, autonomous normal repairs and finite administrative watchdogs remain. No commit/push until root accepts the revised implementation. Send one substantive ready-result callback and stop without polling.
