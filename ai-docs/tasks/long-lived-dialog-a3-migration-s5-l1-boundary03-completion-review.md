# S5.2 boundary03: completed functional checks and preservation repair

2026-10-26. Root reviewed the exact boundary03 source, public evidence and failure,
with an independent read-only GPT-6.1 Sol Max review. This decision authorizes
static preparation below, not another full boundary run or publication.

## Accepted evidence and limits

`F-exhausted-boundary-03` remains **whole FAIL649.592232 seconds**, matching
harness exit101/0passed/1failed after641.54 seconds. Neither controller760 nor
outer work780 expired. The failure is the preservation helper's initial inventory:
`Get-FileHash` cannot read `boundary-phases.jsonl` while the controller retains its
writer. The output directory was never created. Both runtime owners were stopped;
cleanup left zero owned children and root's process snapshot found no Kilogram.

Root verified104 case/source/image/public-export references with zero mismatches,
170 monotone phase records, the ledger sums and semantic equality of all36 prior
cases. Index is empty; diff whitespace checks pass with the existing LF/CRLF notice.

The bound source's sequential `ensure!`/`?` path reached `checkpoint.info/end`.
Root accepts the completed functional assertions: both actual migrations Ready;
identity/nonce/secrets/Native/Session correspondence; no replacement constructors;
exhausted-send refusal before Packet plus exact retry/body conflict before and
after reopen; original527 history; actual B17 receive/ACK; fresh both-indexed
lease/admission/locator/Resume barrier; zero replay; exact frozen suffix; both528
histories; actual FullL1 B17 provenance; cold signed receipt17 and matching
frontiers[512,18]; unchanged frozen original verified raw before and after.

This is source-bound acceptance of completed assertions, **not a whole test PASS**.
Detailed returned status/history/cursor/token objects remained in RAM. There is no
`parent-exhausted-result.json`; do not reconstruct it as an original observation.
Private preservation is still missing. A separate preservation result will not
retroactively turn boundary03 green or restore missing original outputs.

Key immutable evidence in `long-lived-dialog-a3-migration-s5-l1-evidence/`:

| File | SHA256 |
| --- | --- |
| F-exhausted-boundary-03.json | 6da9c4ae43dad5234df1da2aba156e0a972a7d3895069d4cf87fcdbe7932cb48 |
| F-exhausted-boundary-03.log | 739839e858e1a5131e0bbe09063851f4fc2ab193d96297fd2466679b78f4a38f |
| F-exhausted-boundary-03-observation.json | 7f2adda05a3e1a845b4b7580db54e402283fad608e0d2c24de98a7b82f5f05f0 |
| F-exhausted-boundary-03-phases.jsonl | 940df52d347a96dcb48d1f62b7fbe2cdfc5440a62f96868f4872aa86de94c4af |
| F-exhausted-boundary-03-checkpoint-info.json | 2b359e4623b2e603977342df69f441e35e0276643ee420a751a3515e361771b3 |
| F-exhausted-boundary-03-two-indexed-reconnect-barrier.log | 545f0849be3035bb86521b03bd3dce5fd972c472cce603f27ae5c722647c8b8f |
| contributing-cases.json (before new work) | acd66ac604bb83cfee19d99a26122ce5f345b2d4f3eea4d0791e644869251f6e |

## Narrow static preparation

1. Preserve the actual pre-fix controller and runner as source snapshots, and the
   current stable harness at a separate fixed retained path, with byte/hash bindings.
   Controller is40620B/c3e044594d43bcbf60b8055ba203aa8e23220f3b92b35d75f9bbfc049aae0977;
   harness is13935104B/83429f6d41f566aadfdaddea0b862a0fe71120beec875560f6d9463dfdd932d0.
   Historical references stay immutable; distinguish old paths from retained bytes.
2. Remove the persistent writer from `Phases`. Create the file once with create_new;
   each record opens append, writes, flushes/syncs and closes before its action
   proceeds. Keep the same phase/error/deadline assertions. Do not weaken the raw
   preservation helper, skip JSONL, or permit copying a concurrently changing log.
3. Prepare one small Windows C test using the actual `Phases::run` and preservation
   helper on opaque bytes and two genuinely held empty lock files. It must prove
   hashing/copy succeeds during the action, copied phase bytes equal that frozen
   prefix, and subsequent source phase logging still works. No Native/DB/network.
4. Prepare a preservation-only F test using the unchanged
   `obligations::preserve_input` on the stopped original active path
   `E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb`.
   Bind the case03 checkpoint/public exports and exact input path before use.
   No new active copy, runtime, migration, typed DB open, recovery, repair or restore.
   Keep both original absolute profile paths and held OS locks. Create a new immutable
   private backup; never open it. Preserve source-before/source-after/destination
   byte comparisons. Write new result/logs outside the input. Do not touch the
   original phase log or invent the lost parent result. No frozen-origin traversal.
5. Add only the exact new case/selector/deadline guards to the existing contributing
   runner, retaining all old policies and immutable attempts. Use planned IDs
   `C-boundary-phase-preservation-01` (work30/whole40) and
   `F-exhausted-preservation-01` (work120/whole140). F requires C whole PASS and
   at least365 seconds remaining (whole140 plus225 reserve). Require exactly one
   matching test PASS and normal wrapper finalization for each new case.

No product source, product-image rebuild or broad suite is needed. Keep the raw
helper and obligations preserve function unchanged. Static fmt, strict scoped
Clippy, compile-only and PowerShell parsing are authorized; use locked/offline
verification, Jobs12/BelowNormal, fixed stable paths and no live-image overwrite.
No contributing C/F run is authorized until root reviews the preparation. Do not
change any source/script during a contributing run. Use ordinary sandbox execution
where possible; the user's standing permission does not bypass tool review.

## Accounting and next boundary

Current C147.8846192/F3263.7458077/P0 under500/3650/0; no budget amendment or refund.
F remaining386.2541923 covers140 and leaves246.2541923. Historical Ready02/capacity01
split outcomes and all failed boundary attempts remain exactly recorded.

Submit a concise readiness note with source delta, retained provenance, static
results, exact selectors, bindings and commands. Root will review before the C/F
go-ahead. After successful preservation and its review, finish the original S5.2
implementation report and exact candidate allowlist, explicitly retaining all
limitations above. No commit/push, cut3 or whole-P yet. Full rerun is not required
solely to repeat these already completed functional assertions.
