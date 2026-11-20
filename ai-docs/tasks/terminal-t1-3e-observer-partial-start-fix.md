# Observer loader, partial-start diagnostics and field04 preparation

2026-11-21, review candidate on parent2441dc11. [Field03's unchanged failed result](terminal-t1-3e-sender-crash-field-attempt-03.md) is included in this scoped publication candidate. Real runtime137/reconciliation occurred there; recovery did not pass. No installed activation or publication was performed during this preparation.

## Minimal corrections

The observer's crash_restart branch now validates PlainFile, reads common once, requires exact accepted SHA256 `02b2a53e5b9d13f52312059ab1fc8a859de43eb694be72ce0eca00b06fe6346a`, and loads a ScriptBlock from the **same verified array**. Its stale-descriptor/session/context/owner/reconciled-fault predicates are unchanged. Checked observer invocation still has no Bypass; no policy setting changes.

OfflineRecipient and the custom SenderCrash.start register a chat incarnation with unknown PID before ready/status/preconnect can fail. Only a successful verified preconnect binds the chat PID. The inherited and crashed shutdown collectors share a lifecycle guard: partial/unregistered scope produces explicit incomplete coverage and a separate capture error, once, without a diagnostic request using an unknown or older terminal's PID. An unregistered scope is an explicit invariant failure, not a swallowed KeyError. Collection errors stay separate from the application primary and ordinary owner cleanup errors. Known scopes retain their existing typed collection/expected-crash rules. No owner is reclassified as clean and no missing diagnostic is marked complete.

Sender-crash entry changes only fixed attempt/evidence03→04. Product/runtime protocol and EXEs remain unchanged.

## Focused evidence

Evidence root `.tmp/terminal-t1-3e/sender-crash-observer-prep-01`.

- `native-02`: **15 actual external observer cases PASS**, full observer_command gzip/stdin, PS5 process Restricted/no checked Bypass, existing policy Fixture/reconciliation and isolated native observer process pattern. Fixture redirects only fixed A base/IP, uses owned non-network fixture CLI and real command-line/PID/start/token readers. Original observer fails PSSecurityException; corrected exact stale-descriptor proof succeeds. Wrong common (including executable marker), contexts, session, descriptor and unproven reconciliation refuse. Test-only after-hash disk replacement proves verified-array execution. Post-connect status2/full-status frame, exact SHA/bytes, real current-runtime identity/token, bad image hash/start/PID, absent descriptor, identity and ports paths are probed. Protocol replies are synthetic; this is not product crash/recovery proof. Guard100/115 ended with empty Job. Only own retained fixture processes are stopped.
- `partial-04`: **12 lifecycle cases PASS**, executing actual inherited run/finally/receipt/collect_shutdown chain with synthetic IO. Frozen published classes reproduce the masking KeyError for ready/status/preconnect across old-A custom, new-A custom and inherited B starts. Corrected classes preserve primary and separate cleanup/capture flags, unknown identity coverage stays incomplete and no stale/replacement PID query occurs. Additional cases cover known-PID diagnostic IO, crashed override with unbound identity, and explicitly unregistered scope. No actor/fault operation occurs.
- `positive-capture-01`: existing per-incarnation positive capture test PASS; old/new diagnostic PIDs remain correctly separated. Existing crash model's six test methods also passed, including strict crashed diagnostics, shared-budget refusal, one-use/lost-reply, recovery failures and notification binding.
- Failed fixture preparations are preserved: partial01 missing AST location metadata, partial02 incomplete synthetic plan pins, and native01 expected a narrower descriptor error name than the unchanged classifier's actual LAB_CRASH_OUTCOME. Native01 still ended with empty Job. Native02 fixes only the test expectation and the dot-sourced library parameter scope. Earlier successful partial03 (11 cases) is retained; partial04 adds the explicit unregistered invariant case.

No Rust/product build, broad campaign or physical field was launched.

## Bounded remaining external-chain audit

This checklist concerns only actual sender-crash callpaths, not a general security audit. Source locations refer to the current files; branch names are authoritative if line numbers shift.

| Callpath | Nested loader / context | Evidence and disposition |
| --- | --- | --- |
| Agent ready/begin/status/owner/end/cancel/update | lab-control Agent.invoke verifies script/config, invokes agent file with explicit process Bypass; agent file retains real PSScriptRoot | Existing actual field03+metadata operations; source checked. No ambient external file-policy dependency. |
| Native attempt/owner launcher | lab-agent Start-LabProcess and native owner encoded launch use explicit process Bypass; native host runtime_crash file common loader runs in that context | Actual field03 runtime137/reconciliation; source checked, unchanged. |
| Host init/clock, descriptor/fault | Host.command checks host hash then ScriptBlock; clock is Check-Image plus ScriptBlock; external crash common is verified read-once bytes | Prior accepted nine Restricted host cases remain applicable: host/common unchanged. Actual field03 descriptor/fault/clock passed. |
| Host mailbox RPCs: connect/watch/request_status/history/send/quit | Check-Image mailbox.cs followed by Add-Type over source text; operation frames are data, not separate script loaders | Actual pre-fault field03 RPCs; post-recovery uses this same compiled bridge. Source checked; no new file-loader branch after connect. |
| Host put/get/export/socket/receipt/diagnostic-read | Plain/capped JSON/bytes and content pins; no new PowerShell file evaluation | Actual field03 transfer/receipt/typed old-A diagnostics and existing two-incarnation reader checks; source checked. |
| Observer identity/network/ports | observer_command is gzip stdin→ScriptBlock; identity is native WindowsIdentity; network is compiled NLM+absolute netsh, ports absolute netstat | Actual field03 preflight; new Restricted identity/ports probes. No nested script load. NLM/profile settings are read only. |
| Observer preconnect including postcrash stale descriptor | Only remaining implicit file loader was crash_restart common dot-source | Fixed here; original fails/corrected succeeds via actual external gzip transport, with strict negative bindings. |
| Observer post-connect/recovery status/full_status | Absolute pinned CLI status2 with absolute IPC file; real runtime PID/start/image/token checks; no nested script loader | New Restricted status/full-frame and negative identity/hash probes; fixture CLI reply is synthetic. Actual product recovery remains a future field gate. |
| Diagnostic collection/final notification reconciliation | Scoped host JSON reader; Python in-memory typed records/events; no per-incarnation script loader | Strict expected crashed record checks unchanged; partial-start primary masking fixed and regression-tested. |
| Metadata Apply/Restore/readiness | Existing pinned runner with explicit process Bypass; existing fixed file objects/manifests/ordinary token/DPAPI gates | Actual default READ-ONLY Apply Validate below; no stop/Wake on this preparation. |

No further analogous implicit nested file loader was found in the remaining external scenario paths. Existing native file/PSScriptRoot assumptions have explicit launch context; this statement does not claim unexecuted product recovery has passed.

## Concrete packet04

`packet-01/apply-plan.json` SHA256 `8a593af9bfd01cc3d451fff1cbba077e1e0f5b93462821be794a3e5de0068912`: current original A PID13052/start134396791065793360/session6022205168bf4fd3a58a9645bf6ceef0, fresh refresh-a-03, runnerdb3cd102/helper190eb4c4, canonical15 and fixed old/new manifests unchanged. Only needed observer source binding is updated in the refresh plan. Actual default read-only Validate PASS proves current old15/no policy/native+raw/task/idle/canonical15. B fresh read-only original6/same-incarnation idle/readiness also passed. Proposed refresh-a-03/restore-a-04/field04 paths are unused.

`field04-plan.template.json` SHA256 `75995150f55a5b39f483e6bf519c3c98c0aba00046c74952a1c0c4d1ed9afc0f`; `restore-plan.template.json` SHA256 `680e7c7492b912aa6a342ff3783593557ce6e177e6bd19c44cfffe08a9bad948`. Future seven A identity/token/proof fields and reverse expected_actor3 remain NULL. All30 source pins and unchanged candidate/original asset cohorts/product images are verified. Host asset remains accepted5a5a9287. Full A host commands with maximal integer fields still fit3752/6832<7500.

The compact packet reuses the reviewed previous materializer/field-runner structure, with current parent2441, new plan/source bindings, fresh refresh03/field04/Restore04 paths and publication checks for the newly affected source/test files. Materializer derives only actual identity from successful fixed Apply, requires actual new-session DPAPI, current A reverse Validate, fresh exact A/B original6/idle/readiness and full static/structural checks. Eight mock-only derivation/refusal cases pass; future installed-success branch is intentionally unexecuted. All acceptances remain false/commit NULL.

`commands.txt` gives the bounded next sequence for one conditional review: exact source publication; default Validate then Apply180/200; actual-success materialization; one field04 at1800/1860/BelowNormal/max2/native1000/1015reserve60 with unchanged functional/parent/one-use/history/notification gates; conclusive both original6 restoration; derived metadata-safe Restore04 at180/200 and independent canonical/ordinary readiness checks. Known-state reverse only, no force/old File.Replace/adoption or source changes during activation.

Current A remains healthy idle originalold15/no crashpolicy. Failed01/02/03 and original field03 result note are immutable. Scoped source/result candidate awaits root review; no commit/push/Apply/Wake/field/Restore at this preparation boundary. Then idle without polling.
