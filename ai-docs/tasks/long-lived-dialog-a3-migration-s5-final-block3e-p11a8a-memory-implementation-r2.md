# P11A8a R2: missed slots and immutable finalized evidence

2026-11-02. Parent `546487d9c8001c6d110444a05acb3c0d1c381772`. Follow-up review4631B/SHA256 `d860ae1dadec62004b5610f4a37f582ea337205aba0c9c57f9f3dcb1fb555521` verified. This revises the first A8a candidate, not an acceptance/publication. Only memory ledger and its pure tests changed since first submission. A7 capture/query/fixtures, product/Cargo/scripts and shared Controller/Node/Pair/normal/relay hooks remain unchanged. A8b/whole runtime, messenger/network/private fixtures, commit/push remain inactive.

## R1

BeforeStop now consumes overdue live slots once and advances the existing due cursor. Accounting cutoff is the boundary attempt's start; query cost/uncertainty remains in interval/gap fields. A boundary exactly on a due instant services that instant; a late boundary leaves all earlier due instants missed. Start0 -> no progress -> BeforeStop20001 now reports4. A prior progress query consumes its own prior interval; later boundary settlement starts from its updated cursor, so nothing is counted twice. Existing5-second/no-catch-up cadence,36/1032/two-owner/2400s limits and memory caps are unchanged.

Normal closing time is intentionally disabled and remains an explicit closing tail, not missed progress. Missing-boundary stop requests and unexpected closes also settle remaining eligible slots without inventing a boundary sample; those outcomes still fail. Expired cleanup accounting is capped at the original observer span while its actual close/error facts remain recorded. No extra query, gap cutoff or timeout/workload change was introduced.

Three new pure regression functions cover late/no-progress boundary (including exact5000/20000 and late20001), prior progress plus late stop (including an already-missed interval), and long normal closing tail. A 20001 pre-stop then30000 close retains4 missed slots and9999ms tail, not additional post-request misses. The existing cadence/full-count/failed-cleanup cases also pass.

## R2

complete_at rejects sealed before its outer error handler can change any row/inflight flag. request_stop_at and close_at likewise reject sealed before touching clock, lifetime or functional failure flags. Other mutating entry points already go through checked; the regression exercises them too. Active-run errors still poison coverage, and actual cleanup after failed observation or failed finalization remains recordable before any successful seal.

The new regression saves a Copy Attempt, successfully completes/stops/finalizes, then replays completion and attempts stop/close/register/bind/due/begin/Stats/checkpoint/finish. Every call is rejected. Serialized rows, diagnostic and full lifetimes plus stored clock/failed/sealed flags remain byte-equivalent. Failed-finalization cleanup still records its actual waited outcome without becoming valid coverage.

## New-image verification

Revision-specific raw base `.tmp/s54-block3e-p11a8a-r2-01/`; no original receipt was overwritten.

| Check | Actual child exit | Seconds |
| --- | --- | --- |
| fmt-01 | 0 | 4.198 |
| clippy-01 | 0 | 10.854 |
| compile-01 | 0 | 98.265 |
| pure-01 | 0 | 17.558 |

Edition-correct format -> strict Clippy -> stable compile-only ->10 exact memory pure selectors, all PASS/0fail/0ignored; six aggregates select0. Six previous ledger/parser checks and four new R1/R2 regressions ran. No13 A7 replay or real sleeper/query rerun. Jobs2/BelowNormal/offline/locked/x64 verification-scale; existing finite1190/1200 compile,590/600 lint,230/240 format/test watchdogs. Original bounds were not widened.

The new fixed-path migration image is 18025984B/SHA256 `52e605eb54fe026c5426ee2af5521afb32a0a3520a62668c3f0cbd4d17b1476e`. Compile/run before/after inputs match;501 units/all501 fresh,593 current compiler source inputs and independent frozen guard verified. Actual pure wrapper returned0, work_expired=false and empty Job; no query admissions or OS fixture launches occurred. Compile receipt separately records its remaining member before cleanup and verified empty Job afterward. Nine other compiler images/four field images remain unchanged.

## Historical OS evidence and exact handoff

First revision remains immutable: original48-path manifest14175B/SHA256a5bb2e23d797c9b5ff3da6a8d285a1bcf55e90233e487d3084c1e1cf7cb54b5d; review85235B/SHA25609c6756b28a4a72e83e2d82a93015fe86b0d386f1c24dea81d49c871213f4b08; original logs/raw captures/case files/evidence archive untouched. Before edits, all7 first-revision sources and the actual executed18026496B image/SHA256b2a82f5bdfd96362bc3e2707e440a19135d445d162f25477ec8373b49cbeeea4 were copied and hash-verified under the new base's first-revision directory. Source-to-archive mappings preserve the two now-replaced source pins. That first image, not this new one, owns the historical20 PASS/9membership/three real memory query evidence already independently confirmed by root.

Five unchanged source/fixture dependencies and all original tool/fixture pins were rehashed, including query/capture/pipe, benign fixture, exact System32 memory shell and bundled resident shell. Historical OS evidence supports those unchanged paths; it is explicitly not a new-image OS execution claim. Pure clocks/identities/Stats remain injected fixtures. No whole-profile memory coverage, terminal-inclusive/unsampled peak or whole-run fit is claimed.

The revised exact manifest retains the original supporting artifacts, current seven source paths, original A8 task/proposal/decision, root follow-up review, this report and one revision evidence directory. Executor review binds current compiler/pure artifacts and separately labelled historical OS image/case. Full captures/expected manifest/inventory stay at their immutable raw paths; no raw log rewrite. Documentation generation is excluded from frozen execution inputs. Index stays empty; unrelated navigation/historical work preserved. Stop for root implementation review; no commit/push or A8b without acceptance.
