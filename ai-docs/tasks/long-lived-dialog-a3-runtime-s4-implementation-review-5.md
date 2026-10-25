# A3-S4 final implementation acceptance

Root review, 2026-10-25. **Accepted within the fresh S4 scope.**
This is the final implementation decision after contract review and repair
reviews1–4. User authorization covers ordinary commit/push after this acceptance.
The executor is to publish only the exact final allowlist, without more source,
evidence, formatting or status changes.

## Accepted result and independent checks

Baseline HEAD/local origin/master and actual GitHub master were independently
verified as `9547ed171c64bf5ea529b4ba487acf6aa1a61b5b`; its sole parent is
`7f82036ee5803edd64be98b73572604250f5961c`. Index was empty at submission.

Root reviewed the production paths and contract invariants across earlier reviews:
purpose/version fences, authenticated enrollment, Q/current-policy/lease separation,
Native trial rollback and atomic history/receipt commits, durable request/debt and
bounded owner scheduling, restart locator lifecycle and explicit local routes.
R1 loopback enforcement, measured empty-turn read removal and both Windows helper
repairs were reviewed explicitly. No remaining blocking finding was identified
within this accepted scope. This is a scoped engineering review, not a whole-product
security certification.

Final complete P `p-full-review4-relay-repair-1` passed in2003.5355583s runner and
1979.794s controller, under the original2400s target. The explicitly authorized
3000s ceiling was not needed above2400. A single fresh fixture completed every
assertion; no earlier prefix or smoke suffix was used to assemble a PASS.

Root independently read the final P assertions/status/history output and recounted
the actual child logs:1640 Text plus4 genesis,3288 committed calls and3288 attempts,
4 Account/4 Session constructors,24 bindings,63 normal C1 pages,1 full64 funded head.
All expected restarts,20 duplex DH rounds/late tail, real offline256 accumulation,
full pending window and concurrent C2, lost nonempty ACK and B-only relay delivery
completed. Both peers have final C1 receipts936/640 and C2 receipts32/32 with all
queued/pending/debt counters zero. Exact final histories contain1576 and64 Text per
peer, with the old624-row cursor suffix preserved. Root recounted311 private-memory
samples with maximum36536320 bytes.

Full F passed19 cases. Conservative successful F contributors total620.4258161s
within1200s; components total370.2900657s within600s. Their109 observed executions
include3 NOT_SELECTED scale sentinels; no100k/long corpus claim is made. Source-scoped
reuse is accepted: the later helper/controller change does not alter exercised F
paths; the earlier S4 snapshot read change is outside legacy ComponentRoot5/history
tests and is covered by current foundation/component/F checks. All nonempty owner
guards remain covered on the current CLI. Final strict six-package/all-target Clippy
and static checks passed. Existing failed/incomplete attempts remain FAIL.

Root independently verified the final source manifest against current files (all54
rows, including52 Rust and2 PowerShell files), exact helper/controller deltas and
fixed image hashes. The application remains d5d8c8c118925dfde67e37fdb32aa5a120293cde258e93843dd1939fddb15560;
the repaired relay is b8723b8cb546a0ceccf3179415d580b073f0a36d2f9a458ecfd15c66bf2c6317.
Previous L1/predecessor images, Cargo inputs and the clean development-environment
document remain preserved. Targeted publication-path/artifact checks found no vault,
token, private checkpoint, EXE or unrelated path in the candidate.

## Evidence and exact publication boundary

Executor report SHA256:
`2b0df1da1087036b483f31170cea961474106e31e6bf2529815d959fad04f13d`.
Whole P metrics SHA256:
`c5eb48c35d18a0f21bd79e63518bc4605db605d6cb7b08071dc344a02ceff48f`.
Executor candidate manifest SHA256:
`01540c0bba5a294139cee055afe0835a168fa3b81919b67a5c42162c1a950fe2`.

Root independently checked all524 candidate/reference rows for raw bytes/SHA256 and
normalized Git blob identity, and all525 submitted changed paths including the
self-excluded candidate manifest: no mismatch, duplicate, missing or extra path.
The executor report, candidate manifest and root-documents-reference are immutable
submission snapshots. Subsequent root-only README/coordination/next-stage updates
and this acceptance are deliberate additions; the final accepted manifest records
their current hashes, rather than rewriting those historical snapshots.

Authoritative publication list:
[implementation-accepted-files.json](long-lived-dialog-a3-runtime-s4-implementation-accepted-files.json).
Its own bytes/hash/blob are supplied separately in root's publication instruction
to avoid recursive self hashing. Commit only that manifest plus its listed paths.
Verify exact staged path set and normalized blobs before committing, then exact
committed path set/blobs and sole parent after committing. Use one normal commit
on master and normal fast-forward push to origin/master. No broad add, amend,
reset, force push or unrelated changes. Return commit SHA, push result and clean
worktree/exact-path checks. Root will independently verify remote publication.

## Remaining boundaries

This accepts the fresh two-process S4 runtime exercised on loopback/direct and own
relay. Existing L1 migration/handoff (S5), public/multi-network field operation,
standalone onboarding, GUI/groups/multi-device and whole A3/MVP are not completed
by this stage. Deferred A2 crafted relay DoS and unchanged legacy OD1 Windows
pause-hook audit retain their existing status. No next stage is launched here.
The measured verification time is not release throughput, and logical quota/IO
fault tests do not claim a physically full disk. Full development cost/failures
remain visible in the ledger rather than being hidden by successful-set accounting.
