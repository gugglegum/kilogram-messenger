# Offline recipient initial-history correction: source and retry preparation

2026-11-20. Implements the narrow [accepted correction](terminal-t1-3e-offline-initial-history-fix-task.md) following the [failed first attempt](terminal-t1-3e-offline-recipient-live01-result.md). The first attempt remains failed before offline delivery; its evidence is preserved. No new field execution or product correction is claimed.

The accepted exact1 failure note was published as `62f64f5ff9d133da1e3cf1c52a58c11d753750c0`, parent `9a8b280c1267a8a999b5b2a7e0d023ecbed8d640`, exact path/blob/index and live origin verified. Publication proof `.tmp/terminal-t1-3e/offline-recipient-result-publication-01/publication-final.json` SHA256 `b88fc0c0a062e8f8276a459202bac200c3c1539487ebdf31d56e01bc3502a633`. Foreign8 and9951 other notes were preserved. This correction candidate is uncommitted on that published base.

## Corrected behavior

`Field.collect_histories` contains the existing inherited collection expression: both hosts, all installed contexts, the existing checked terminal `history` operation with its context/finished/single-page requirements. Final `histories` now calls this collector and the **unchanged** `verify_final_history`, then saves its successful result. No post-delivery invariant is weakened.

`OfflineRecipient.initial_histories` uses that inherited collector, saves the complete collected snapshot as `initial-history.json`, then applies the explicit initial validator. It requires no previous application messages, exactly two distinct installed context IDs, exactly both distinct expected hosts, each expected context exactly once on each host, and empty list rows throughout. Expected contexts come from fresh Setup, never the empty delivery list. A collected but invalid snapshot remains available for diagnosis; failed/incomplete collection cannot pass. The inherited final-delivery verifier is not called with zero messages.

The candidate has four publication paths: this note, `scripts/test-terminal-t1-3e-driver.py`, `scripts/test-terminal-t1-3e-offline-recipient.py`, and its check script. The driver was an existing **untracked, pinned dependency**; publication would add its complete source to Git. The focused delta against the previously reviewed package shows only extraction of the collector and its call from `histories`. All unrelated existing driver code and the final verifier bytes are retained. The complete base diff and focused three-script delta are both supplied for review.

## Relevant checks

Eight focused methods passed via `--initial-only`:0.032s suite/0.328s command, raw output and exact four input hashes in `.tmp/terminal-t1-3e/offline-recipient-prep-03/check-run-02`. The prior seven-method development run/fixtures remain in `checks-01`. No Rust, network or unchanged PS reader suite was run.

Tests exercise the actual inherited collector and production initial validator with fake terminal/host I/O. The valid two-context empty case passes; the former final verifier still rejects this same zero-message snapshot. Missing/extra/duplicate installed contexts, missing/extra/duplicate/wrong context or host scope, missing/malformed/nonempty rows and prior sends fail. Real collection also rejects wrong-context or unfinished replies, missing items and I/O failures. Successful invalid collections are saved before rejection. The complete functional sequence uses real initial **and final** history methods: no `histories` override or patch and no history-function patch. Invalid initial I/O stops it before recipient stop/send/restart. Its valid sequence reaches pending two, recipient-only restart, four delivery records and strict final histories. The pre-anchor partial-notification case also still passes its explicit partial-coverage verdict through this sequence.

Final-history regressions use the inherited collection and unchanged final verifier to reject missing/extra/duplicate rows, changed event/text, differing peer row data and missing context scope. They preserve the valid exact event/context/count case. These checks are synthetic helper execution, not a physical offline success.

## Proposed single fresh attempt

Packet `.tmp/terminal-t1-3e/offline-recipient-prep-03`, plan `package/runtime-plan.json` SHA256 `ef30ad8278060c24e97684eaf8a290722e93cab652486c3016c9e0b4515f6ac4`. Pinned wrapper SHA256 `75652571e28d724f8517aa298540429b6d20c9f16d72045cf04be98a1f28b026`. It validates one **hint-offline-02**, evidence `.tmp/terminal-t1-3e/offline-recipient-live-02`,4 messages,1 recipient restart and0 host mutations. Old live01 is neither reused nor rewritten.

The offline scope, images, agents, rules and budgets remain the previously accepted ones: fresh synthetic Setup/Auto/two contexts/original90s gate; clean B recipient stop and socket disappearance; A materializes one stable durable pending message per context with two observations while B is stopped; B restarts at the same profile with a fresh runtime/advanced own revisions and fresh Auto90s gate; both original queued events deliver, then one online reply per context; exact histories and distinct notification-coverage verdicts; full cleanup and original-six restoration. Native1000/1015s shared budget/cleanup reserve60, controller1800/outer1860, BelowNormal/max2; Connect85/probe60/minimum2. No extra send, sender restart, network hold, artificial notification or changed handshake.

Both online reply notifications and queued events absent at the first post-watching history remain mandatory exactly once. Already-present queued events without observed notifications retain explicitly unproven anchor coverage; every observed notification must be unique and exactly bound. Final-drain reconciliation and raw notifications evidence are unchanged. Existing A2 images and588-input build manifest are reused; no rebuild. Unchanged reader and notification checks are reused from prior frozen packets except the affected full-functional partial-coverage regression described above.

Source snapshots/pins cover21 helper inputs; only driver/adapter/check changed from prep02. Prep02's78 inventory-pinned internal files remain unchanged. Of341 failed-result pins,339 remain unchanged; the only revised external current files are adapter/check. Failed live01, controller logs, typed cleanup, original restoration and final-agent evidence remain intact. Unchanged host reader remains SHA256 `e0637eeec14a427b5e1c2aa85e33375da497a7e35e93471f5bd1ba210a2584a1`.

No readiness refresh was performed for this source correction. `readiness-reuse.json` identifies the last final-state observations A01:19:48Z/B01:19:59Z, exact hashes, and their ages at packaging01:28:48Z (A540.8/B529.1s). They are explicitly aged, not confirmed-current availability. Eventual live preflight must again confirm exact agents/idle/originals/network/process prerequisites; it must not substitute these old observations. No deploy or live02 execution occurred.

Local validation from the repository root:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' -B '.tmp/terminal-t1-3e/offline-recipient-prep-03/run-bounded.py'
```

Submit source/tests/packet together for combined source/publication/runtime review. `runtime-acceptance.template.json` remains false/false and requires an accepted exact publication before materialization. No commit/deploy/run is authorized by this preparation. Return the reviewed failure-note publication proof and this concrete correction packet once, then idle.
