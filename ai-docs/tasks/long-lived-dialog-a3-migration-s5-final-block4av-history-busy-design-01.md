# Block4AV: executable history-busy regression contract

2026-11-10; source-only design on `8b9bb7024e39de2bbd5c869723f2b737c0c96ac9`. No implementation, compile/tests, profiles, Prepare/Run, new F or publication. AU raw scope stays closed. F11 remains failed/consumed; its32-ref holder and eventual release remain unknown.

## Recommendation and unavoidable seam

Use a **real retained ACK dispatch**, not a reserved toy counter. `Owner7::ack_push2` (runtime7/protocol2.rs:972-1014) reads actual sources/receipts, reserves32 and returns `Outgoing7` containing `DispatchExisting7._refs`. Retain that returned object outside the owner callback. Release with `drop(held.take())`; observe the same Owner7 window returning32→0 through its mailbox. No new production control is needed.

`hold_ack` also holds the dispatch: network2::rpc:148-163 invokes verification_ack before sending, while borrowing the dispatch; verification.rs:409-448 and Pause::wait_until:271-290 publish an ack marker and poll the existing control file. CatchUp/hold_ack=true,hold_ack_from absent works for A; a present hold_ack_from requires B/push (receipts.rs:93-101). Clearing hold_ack releases the pause, but the permit survives until the subsequent RPC/Complete and dispatch drop. Therefore marker disappearance alone cannot prove release. This route needs observer configuration, a live RPC and additional control handling; retaining the real dispatch is the smaller deterministic admission regression.

There is **no existing fixture joining exact two-context warm40, actual CLI and the F Controller**. `handoff_tests::Fixture::new(false)` creates one context at[3,3]; it has a128-share, so it cannot reproduce the failure unchanged. Its `Net` has slot0 literals. Conversely `Controller::new_feasibility` installs activated capture/memory and `OldOrigin::create_for` enforces the allocated F namespace (control.rs:310-332,p5.rs:86-106); borrowing it for an ordinary regression would bypass admission. Do neither.

Recommend the explicitly separated A/B/C checks below. A proves real storage/Owner7/production IPC-handler admission, B proves actual CLI encoding plus the same reader, C proves scheduling/progress rules. They do not constitute one end-to-end Owner7→CLI→Controller run. The missing link is concrete: Fixture/Net/Owner7 are library-private and unavailable to migration_s5_process; Net has no RuntimeIpcServer endpoint. That combined claim remains for a separately allocated F after review. Adding an Owner7 server bridge and a second bounded CLI launcher inside the library fixture would broaden this repair; do not add them silently.

## A — real warm40 Owner7/IPC-handler fixture

Extend the existing **test-only** Fixture factory with a history-geometry option; preserve existing constructor defaults/cases. Extend existing Net helpers to accept a stored physical slot (default0), rather than clone their TLS/actor/cleanup machinery. Reuse `ipc_value` (handoff_integration_tests.rs:1117) → `runtime::ipc`, which is already pub(super), for real production admission/JSON pages. This layer makes no socket/CLI claim.

Concrete setup:

1. `repair_tests::Pair::new_in` enrolls the two real devices. Add a second real context with the same `admin::create_context/import_context` exchange used by Pair::from_temp:61-69. Designate the larger committed context ID as logical C1, the other C2; assert actual repository mapping C1=1/C2=0 after migration. Run Pair::admit/activate for each context by setting Pair.c; leave Pair.c=C1. Both contexts must have genuine Ready1, same A/B device ordering and roles0/1.
2. Reuse `send_old` for A1,A2,B1,B2 with delivery/ACK. Queue A3,A4 through the real old Engine without materializing. Reuse Fixture's per-owner `storage_migration::upgrade` → Profile3::load/open → Owner7::new, enabling **both** committed contexts. Verify cold imported C1[3,3],A queued2,C2[1,1], then initialized warm metadata/history identity. Do not fabricate lanes or load a prebuilt profile.
3. Reuse Net::legacy and real L1 leases/actors to materialize/deliver the migrated A3,A4 and reach[5,3],all counts0. Fixture's existing signed Consent2 preparation may precede this component phase, while mode remains L1; this is not a claim to replay the whole F order. Invoke the existing propose→prepare→select→commit_b→activate sequence from Fixture::complete through the two owner handles at the resolved C1 slot; reconnect and exchange Net::head. Require genuine Active,cuts[5,3],both next[5,3].
4. Queue A5,A6 and B3..B34 via Owner7::queue; use existing actor materialization and Net::deliver (outgoing2→network2::rpc→check_complete2), in bounded batches. Require both[7,35],40 genuine sources: imported A1/2+B1/2; full-L1 A3/4; Event2 A5/6+B3..34. C2 remains L1,[1,1],counts0. Existing Net mixed mode has incoming handlers and explicit outgoing calls, not the autonomous established worker; defer A's ACK-push progress so ack_push2 returns Some. Assert this prerequisite, never edit ack_next to manufacture it.
5. Capture `held = handles[A].call(|o| o.ack_push2(slot,binding))`, requiring a nonempty genuine AckBatch and window current[0,32]. If needed, clear both owners' remaining debt using existing Net::acks queries; ACK replies use another32 and fit64, without advancing the retained sender's push cursor. Require next[7,35],counts0 on both,held still32 and no new native effects after the baseline.

Call-flow: `held real Outgoing7` → `handle.call(ipc_value([3,4,1,None,16]))` → exact SchedulingWindowBusy before repository.history → `drop(held.take())` → `handle.call(window.snapshot)` current[0,0] → same IPC request → first16,real view/F[7,35]/cursor → reacquire real ACK dispatch → suffix request with that unchanged cursor/limit32 busy → drop/observe0 → exact suffix24,finished=true,same view/F and40 unique original sources. Require ACK-push eligibility again before the suffix; fail fixture setup if absent.

Keep the held object in an Option whose guard drops before Net, including error/unwind paths. Normal teardown drops all dispatches, checks both windows at baseline, calls Net::disconnect (close bindings,abort+join tasks), then uses existing Net Drop stop/join for owner threads/endpoints. On terminal IPC/parse/liveness error perform the same owned cleanup; cleanup failure stays failure. No global environment mutation or observer needed. Add a free-window C2 request during the C1 hold: it must succeed on its independent slot0 share. Repeat the small admission check with roles reversed/opposite resolved slot, rather than a geometry cross-product.

## B/C — actual CLI and the shared F reader

New F-only reader module has an immutable per-page Request(slot,limit,cursor) and typed `Page(Value)|Busy|Terminal(closed reason)`. Its attempt adapter uses existing `command(current())` and `s53_deadline::output`, not cli/json_cli, so it can inspect real status/stdout/stderr before the general helper makes failure fatal. Busy requires **unsuccessful actual exit AND empty stdout AND exactly28bytes `Error: SchedulingWindowBusy\n`**. LF is the observed canonical encoding; do not accept CRLF/whitespace/substring variants without a newly justified encoding. Successful malformed/over48KiB JSON, other errors and stdout-bearing failures are terminal. Never emit raw error/argv/cursor/body.

For B add a bounded protocol-peer fixture inside the existing migration_s5_process test harness using the existing RuntimeIpcServer API and an ephemeral synthetic device identity. It scripts OrderedDialog replies, validates the actual received slot/limit/cursor, and starts no Owner7/DB. Launch the real fixed-path current CLI history3 via existing deadline::output. This proves CLI exit/empty-stdout/canonical-stderr and reader behavior against a controlled server; **its Busy/Page replies are scripted, not storage/Owner7 proof**. A supplies the genuine40-source/page/admission proof separately. No new executable, runner, production IPC API or library CLI-launcher is added. The shared loop/classifier, not a copied retry, is used here and by F.

F ownership/call arrangement:

```text
operation(&mut Controller,owner,logicalC,work):
  node = Controller.view(owner,logicalC)       # owned path/slot snapshot
  Phase::until(min(s.until, entry+30s))        # already caps existing local phase
  effective = s53_deadline::capture_deadline()
  attempt = |request| deadline::output(command from owned node/request)
  progress = || Controller.check()            # sole mutable borrow of Controller
  reader = Reader(effective,attempt,progress)
  value = work(&mut reader)                   # no capture/borrow of Controller here
  reader.check_before_return(); return value
```

Reader.page calls progress before/after each actual command and before returning Page; late success fails. Busy calls progress, yields min(25ms,remaining), then checks progress/deadline again. Each retry uses the unchanged request. Phase and reader span the **whole operation**, not each attempt/page. First16 and suffix24 share one deadline; continuous history's all pages share another single operation deadline. `Controller.check` retains deadline/liveness/decoder/memory/alias/interval behavior; no duplicate binding/progress framework. Command errors already drop the existing OwnedChild through deadline::output; read/parse/liveness failures return immediately, drop Phase and preserve outer Controller-owned cleanup.

Keep Node.history/general cli/P callers unchanged. Factor early::history's existing page-collection validation into history_with(page_supplier); its old Node wrapper delegates unchanged. Route only F's calls through Reader: initial Frozen40,its suffix/resume and continuous40/169/218/C2 reads. Keep all page48KiB/43page/1344row/progress/identity assertions. Frozen40 remains16+24/40 at[7,35]; P remains39/suffix23 and its own frontiers. First frozen resume remains before source64 is armed (feasibility.rs:500-506); never retry around an intentional same-context64 hold or switch slots. C2 under that hold uses its own share.

| Focused case | Evidence and expected result |
| --- | --- |
|A held32/release,then suffix hold|Real Owner7/IPC handler,source-proven16+24,unchanged cursor/view/F; refs32→0,native counters unchanged.|
|A free/opposite slot and C2|Correct mapping/roles; C2 succeeds while C1 admission is busy; no cross-context storage/counter effects.|
|B actual CLI Busy→Page and suffix Busy→Page|Controlled IPC peer,real child exits/bytes,the same Reader; exact slot/cursor and shared deadline. Does not prove real storage.|
|B persistent Busy / nonbusy / malformed success|Original operation bound is not extended; other failures terminal. Always kill/wait owned CLI,shutdown server,join fixture task; no surviving peers.|
|C deterministic clock/progress negatives|Injected time/attempt observations permitted only for late-success rejection,nested Phase cap,one deadline across pages,progress/liveness error abort,and rejection of near-miss Busy encodings. No real-ref/storage/native claim.|

## Exact proposed scope and order

Nine Rust paths, all under apps/kilogram-cli/:

- tests/support/migration_s54_p5.rs — register reader/tests.
- tests/support/migration_s54_history_reader.rs **new** — typed classifier/shared loop,F Controller adapter.
- tests/support/migration_s54_history_reader_tests.rs **new** — B protocol-peer/actual CLI and C scheduling negatives,using existing harness ownership/deadlines.
- tests/support/migration_s54_feasibility.rs — F-only operation/call-site routing.
- tests/support/migration_s54_feasibility_data.rs — Frozen40 uses the supplied bounded reader for capture/resume.
- tests/support/migration_s54_early.rs — extract existing collection loop to supplier; existing Node/P wrapper unchanged in behavior.
- src/ordered_dialog/runtime7/handoff_tests.rs — optional real two-context/queued migration geometry,slot-aware observed helper; default fixtures unchanged.
- src/ordered_dialog/runtime7/handoff_integration_tests.rs — parameterize existing Net slot with default0; register A child module.
- src/ordered_dialog/runtime7/history_busy_tests.rs **new** — A setup,real dispatch ownership,actual ipc_value assertions/cleanup; no CLI bridge.

Implement reader/B/C first,then the explicit two-context A fixture prerequisite. Do not report the repair verified until A and B both pass; if the fixture extension expands beyond these test seams, stop with that concrete delta for root review. The first later allocated F supplies the actual Controller/CLI/Owner7 combination and retained169/218 checks; no extra F or historical matrix is part of this design. Windows compilation without running and stable harness/EXE paths remain mandatory, with root-reviewed bounds/resources. All work here ends at this document; implementation requires the next instruction.
