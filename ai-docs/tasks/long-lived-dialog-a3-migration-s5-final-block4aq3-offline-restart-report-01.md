# AQ3: P1 fresh offline restart and semantic preservation

2026-11-10. Review candidate on published AQ2 `03575a5123c5fa1dc2c9453d5ec5811d0c15623f`. This unit implements only P1 offline restart/semantic preservation and retained-proof handoff. No commit/push, new attempt, Prepare/Run/WholeP or production image rebuild/replacement. P2/P3 warm-current/status work remains separate.

## Preconditions verified before source edits

The actual frozen executable at target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe was hashed, not invoked: 61531648B SHA256 d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1. It equals the public transport-security-a2-evidence/final-build.json record for bc86cd25518077cef8977522e0c750f3bee2c0d7 (build 2026-10-23T19:22:29.9380872Z). Record636B SHA25637655b54a2f8feced9f4e04e72769fc13527dd0051e2ff3fde69e2179eb97387. Only that public record/relevant public Git source were used; no historical private/runtime logs or recursive manifest audit.

The whole runtime.rs blob equals that build revision: fe30077f246999219b4a7585dccf0cdde84f44e3. Targeted Engine::save/native, queue, non-Native Head::bump, Descriptor::issue/parse/valid_now and complete legacy CLI Status arm also match. The relied-on runtime renews both local descriptors via genuine signed issuance, non-Native Head bump/save and atomic public TempFile publication before IPC startup. RuntimeControl begins empty; role0 admission starts immediately and the entire admit_a is bounded by15s before Backoff. Legacy status supplies context/phase/SID/Native and Guard generations/frontiers/pending/unacked, without a queue count or current-format Native keys. This establishes source/build provenance, not an actual fresh failed-admission run. Exact APIs/body hashes are retained in provenance-before-edit.json and provenance-specific-apis.json.

## P1 and shared comparison

P1 retains old A's registered start, then stop/wait/reconciles B and A through the existing Controller. After stopped_reconciled, separate legal exclusive source captures retain B witness/latest local descriptors and A baseline. Original genesis identities are checked without replacement. A requires both phase3 contexts, C1 own/peer next17/16, C2 next1/1, empty queues/pending/debt and no C2 outbox. B's known C1 B16 debt is preserved; the empty-own-debt prerequisite applies to A.

Snapshot validates the actual public legacy profile: exactly the two logical enabled contexts, state/IPC/locator associations and loopback bind. Both stored local descriptors must equal their actual latest public files. A's peer bytes must equal the stopped B's actual latest stored/published local bytes, parsed with the existing signed Descriptor parser for the correct context/role and revalidated for time at setup, observation and final comparison. Signature validity alone is insufficient; a valid older/wrong B target fails equality.

Only A restarts through existing start(0): unchanged controls, IPC/locator removal, owned spawn/memory hooks and interval registration. The small Life association checks old actual waited/reconciled legacy outcome, increasing ordinal and anchored new start/handle; the existing Ledger owns overlap/unawaited/PID rules. PID reuse after wait is legal. The one existing wait30 is clamped to original until. Each poll requires B stopped and the same fresh owned A association, then both actual IPC rows with Backoff plus expected context/phase/SID/Native and Guard generations/frontiers/unacked0/pending0. Existing check surrounds observations with alive/memory/deadline checks. No two-sided import while B is stopped and no new heartbeat/timeout. Queue0 follows the stopped proof and the absence of any queue producer before send17/send18.

After exactly those IPC sends and the existing final A stop/wait, one Snapshot captures the final AQ2 Proof and semantic evidence under the same borrowed source lease. finish compares exact Native bytes, identity/spec/session keys/D/E/SID, Head semantics except increasing ordinary generation, Guard/frontiers, packets/history/receipts, outbox and empty pending/debt for both contexts. It also compares content-hash/length by path for every other durable record; only Head, intended queue and local locator records are excluded from that general map and validated specifically. Peer B descriptor bytes stay fixed, both local A descriptors must be genuinely signed/valid with increased revision, and public/stored local bytes agree. Temporary Head normalization is a comparison-only copy, never persisted. The inventory digest is allowed to change.

finish consumes that same final Proof through prepare(FirstA), requiring exact A17/A18 sealed queue IDs/bodies and C2 zeroText/queue0. The Controller's private migrate_p_checked hands the resulting Checked directly into the mandatory AQ2 retained conversion/cold path; it does not recapture storage. Original identity/preview source/one conversion/cold comparison precede successful accounting, interval/Ready/witness installation, source binding and current start. Errors propagate. Existing typed71/95 accounting and post-migration suspend_materialization placement are preserved. F migrate, P0/P2/P3/P4/P5/P6 and lifecycle/memory/interval machinery are unchanged.

No lease survives capture, restart or conversion. Snapshot/Proof have no Debug or serialization. Additional durable-record hashes and public profile serialization remain internal; no secret/status snapshot is logged.

## Evidence and limits

The new real semantic test builds fresh identities and two independent contexts with actual Engine/Native handshake/Text/receipts/frontiers, both C1 physical slots. A starts with queue0 via the shared fixture's new explicit pre-queue factory. Genuine Descriptor::issue plus normal Head.bump(false)/Engine.save(None) and runtime-equivalent TempFile publication renew local descriptors; cross import uses runtime::import_locator. A is reopened/renewed/queued through normal Engine APIs. No target Head/Native/frontier is stamped. The observed source digest changes; Native bytes and native-call counters remain unchanged. The exact final retained proof then passes actual cold migration verification. This is a real storage/crypto fixture, not a process/transport admission result.

Per slot36 focused refusals:8 injected legacy status changes (missing context/field, wrong context/generation/SID, connected status, pending or unacked);3 invalid public profile configurations;4 genuinely signed/context descriptor substitutions including expired and valid stale/latest-target mismatch;1 observation-time expiry;20 in-memory retained witness substitutions covering Native/frontier/identity/receipts/events/history/queue/debt/pending/C2 outbox/Guard/local renewal/profile/other durable records/keys/native generation/spec/C2 queue/Head. These substitutions do not modify durable fixture state. Both slots execute the same comparison/status functions used by P1.

The separate injected Life test checks10 refusals including the existing unawaited start guard, missing/not-waited prior outcome, changed/stale handle/start/record/wait and expired original deadline; waited PID reuse passes. It does not create a child or prove actual admission. One existing interval test rechecks frozen-format/dirty-counter/coverage guards. Shared AP FirstA regression passes both slots and existing refusals. Because the shared Fresh factory's queue switch also touches F construction, the existing real F cold/strict-warm selector passes both owners. Unchanged late-B and adapter cases were not repeated.

Static inspection (static-order.json) checks actual P1 sequence, three stopped captures including exactly one final Proof, one extra legacy A start/wait, one wait30, no offline import, same Checked consumption, unchanged common migration order and surrounding phases/files. Source call-count accounting gives planned node lifetimes24, A12/B12 (previous23): P0=4, P1=2, P2=1, P3=2, P4=2, P6=1, P7=2, P8=1, P9=5, relay reopening=2, P11=2. Existing caps32 interval/outcomes and36 memory lives, P11 relative +4 outcomes/+2 restarts, same Controller/clock/Guards and deferred original-A backfill/B-unbound handling remain unchanged. These are planned/static counts, never observed runtime evidence. No current zero-operation stream or premature final B memory coverage is introduced.

Actual full Controller, two fresh failed admissions, current-A/legacy-B transport and subsequent WholeP completion are unexecuted obligations of a later accepted stage. This report makes no partial-P PASS claim.

## Verification receipts and candidate

Fresh .tmp/s54-block4aq3-offline-restart-01, pinned Rust1.98.0/pwsh, offline/locked verification profile, Jobs2/BelowNormal. Compilation uses --no-run; focused selectors execute only through scripts/run-cargo-tests-stable.ps1. Stable image 35954176B SHA256 75ef177a943f98672a2948827c06aae7a4922da7b13130eaef37b1d4ff56bf41 at E:\AI-Projects\kilogram-p2p-messenger\target\s54-diagnostics-source-checks\verification\deps\kilogram-cli-test-migration_s5_process-tests.exe. Strict CLI all-target Clippy uses literal -- -D warnings. Scoped formatting/diff and final successful process inventory pass. Final source/tool/image pins are unchanged through the passing focused tests; the failed earlier snapshot is retained separately.

| Step | Owner/child exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- |
| format-01 | 0/0 | 0.921 | 570/600/30 |
| compile-01 | 1/1 | 15.309 | 3600/3660/60 |
| compile-02 | 0/0 | 34.590 | 3600/3660/60 |
| rust-01 | 1/1 | 77.404 | 570/600/30 |
| format-02 | 0/0 | 0.845 | 570/600/30 |
| compile-03 | 0/0 | 33.664 | 3600/3660/60 |
| clippy-01 | 0/0 | 16.454 | 3600/3660/60 |
| rust-02 | 0/0 | 308.836 | 570/600/30 |
| rust-negatives-01 | 0/0 | 3.993 | 570/600/30 |
| rust-early-01 | 0/0 | 135.430 | 570/600/30 |
| rust-03 | 0/0 | 29.546 | 570/600/30 |
| seal-01 | 0/0 | 1.167 | 570/600/30 |

Two failed steps are preserved without rewriting: compile-01 omitted ? on old_head.encode (fixed before compile-02); rust-01 used create_new-only admin::write_public for renewal (Windows80, fixed to actual runtime TempFile replacement before fresh rust-02). The latter's fresh failed fixture is retained; passing tests use new copies. compile-03 and Clippy validate the final sources. All actual owner receipts record bounded Job membership, waited child exit and stdout/stderr EOF, no expiry and empty final memberships. Forced owned residual cleanup: {'compile-02': [41348], 'compile-03': [49376]}; residual application identity is not established, actual membership and after=[] are verified. Other cleanup is unforced/empty. Exact tool return chunks, selectors, Cargo argv, images and raw diagnostic logs are pinned.

Candidate contains only five support sources (source module registration; P1/handoff; shared fixture pre-queue option; new offline snapshot/Life helper and focused tests) plus this report/pins. source.patch includes tracked changes and both new files and passes reverse-apply --check against this worktree. HEAD/index and existing navigation remain at their fresh baseline. No production code/image/preparation, unrelated dirty work, historical private accounts or expired AM16/AD16 raw evidence was changed/read. Stop for root review; no publication or next slice.
