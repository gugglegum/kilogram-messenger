# A refresh initial validation corrected and verified read-only

2026-11-20. Review candidate on published parent `325361ea643e6acb0c1f6eb59535fd30ce8df2e7`. No commit/push or installed Apply is authorized for this candidate until acceptance.

The prior Validate failure and publication remain documented in `terminal-t1-3e-agent-refresh-validate-blocker.md`. All original source snapshots, failed validate-01 raw output and conditional authorized acceptance are preserved. The correction uses a separate packet under `.tmp/terminal-t1-3e/agent-refresh-fix-prep-01`; it does not repurpose the old acceptance.

## Minimal runtime changes

The runner now initializes Context.initialActor to null before the first production Io.validate callback. The actual Get-RefreshActor prelude reads initialActor and lastStopped while evaluating the identity-check arguments, even when Fresh=false; both must exist before entering that callback.

The next native boundary had another ordinary defect: the runner passed a PowerShell image path containing `/` into OpenActor, whose existing native collector compares the expected path literally with QueryFullProcessImageName output. GetFullPath now normalizes the expected path before comparison. The executable/SID/incarnation requirements and all security predicates are unchanged. These are the only two runtime edits.

Early-context audit is recorded in `context-audit.json`: plan/maps/directory/fromKind/expectedActor originate in the entry point; initialActor/lastStopped and other transaction state are initialized before validate. Actual Get-RefreshRealIo.validate and Get-RefreshActor have no remaining missing early context fields in the exercised path. Stop/Wake phase state remains initialized by the existing transaction sequence.

Runner SHA256 `b0b2dce51326d0bcbee86179f5c337cacc5f99eed9d4cad6d15c34a8d9c5aa87`; check SHA256 `8e8ba35ba91f12c76ea0d1b8b3325d05fcb6ef08f19128a1639331ec130506c4`.

## Targeted evidence

Final native PS5 focused run `.tmp/terminal-t1-3e/agent-refresh-prep-01/fix-focused-03`: exit0,20 scenario groups,60/80 watchdog, source snapshots/pins including the unchanged native collector. Existing transaction cases remain focused on actual synthetic filesystem/locks with mocked actor/task IO.

Two added regressions reproduce old-fails/new-passes rather than initializing the missing field in a mock:

- Under StrictMode Latest, the actual production validate callback and source-derived Get-RefreshActor prelude execute with the same initial context shape as the real entry point. Only external file/task readers are substituted, and the prelude ends immediately before native OpenActor. The original published Invoke-RefreshOperation fails with PropertyNotFoundStrict/initialActor; the corrected function validates read-only. The fixture never preinitializes initialActor.
- The exact OpenActor expressions extracted from original and corrected production source are executed against the test's own PS5 process, with retained creation time and actual current user SID. Original path fails with Actor incarnation/image mismatch; normalized path succeeds. No installed actor is selected by this test.

The expected original failures, source-derived prelude/native expressions and corrected results are retained in initial-production-path.json and native-image-path.json. Earlier fix-focused-01/02 evidence is preserved. No unchanged crash suites or Rust/product build were run.

Real READ-ONLY Validate of the corrected runner was explicitly authorized before review. `.tmp/terminal-t1-3e/agent-refresh-fix-prep-01/validate-01`: exit0,2.266 seconds, timeout=false, empty stderr; default Validate, BelowNormal, runner watchdog180/200, outer capture210. No mocks were used. Actual snapshot SHA256 `f438811cdb3eac27d94bb8820878e29ebac858b3aa1d273b40ba2382610ae707` confirms ordinary native/raw groups and attributes, actual process image/start/session, singleton idle session Job, unchanged task/security descriptor and campaign baseline.

The validated current OLD A identity is PID44024/start134395772085441302/session261956141a2e4d5ca0aa8382f882bad2. This is fresh read-only availability evidence for the old agent, not a refreshed agent or a DPAPI probe. Apply, stop, replacement, Task.Run, DPAPI probe, Restore and physical messenger crash were not executed. Installed old15/config/baseline/original bin6 remain unchanged; B was not contacted.

## Concrete candidate packet

`.tmp/terminal-t1-3e/agent-refresh-fix-prep-01/packet-01/activation-packet.json` indexes the revised plan/commands and real read-only proof. Apply-plan SHA256 `82e9b088ce7e5a5f584e5d2f0feb4ba00f4e184cf70800811c6f3155067bd466`: only the runner source pin changed from the previous plan. Host/SID/identity/budget/old15/new15/config/baseline/security constraints and unused operation refresh-a-01 are preserved.

Both acceptance templates have authorized=false and require the future accepted correction commit. Reverse identity remains null until a successful future Apply; field identity has not been fabricated. Commands include the same guarded forward/reverse procedures with the new packet and hashes. Old authorized acceptance cannot satisfy the new plan/runner bindings.

Return exact runner/check/note diff, targeted PASS, actual Validate proof and candidate packet to the architect, then idle. Commit/push and installed Apply follow only explicit acceptance of this diff.
