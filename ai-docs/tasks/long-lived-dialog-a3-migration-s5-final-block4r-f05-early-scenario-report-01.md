# Block4R: F05 source-guard incompatibility and focused repair

2026-11-03. Baseline `81a61ef3a8ef27cc77315258a2a59ec47e98b7bd`. Established failure: the first `source::Legacy::capture` calls `source::owned`, whose original predicate admits only `.tmp/migration-s54-integrated`. The admitted F05 origin is under `.tmp/migration-s54-feasibility`, so the guard rejects it before `V4Source::open` or any key/source read. This is a deterministic test-controller compatibility defect. No product/protocol/storage/cryptographic code was changed; no production replay occurred.

## Evidence and earliest check

Fresh admin `.tmp/s54-block4r-f05-early-scenario-01`. Bounded local read-only analysis inspected the permitted37 command stdout/stderr pairs and four source-identified initial owned runtime logs, checking canonical paths/reparse status and sizes before reading (command1MiB, runtime16MiB caps). It did not traverse profile/state DBs or read key/seed files. Exports contain only fixed source-backed field names/types, booleans/counters/PIDs, static labels and necessary original pins. Runtime log references use fixed owner/stream selectors without exporting the identity-bearing directory suffix. No arbitrary message/address/identity/key/token value or raw log copy is in the handoff.

`diagnostic-01.json` records eleven successful status captures (indices22–30,32,33) and six successful memory captures16,17,31,34,35,36. All seventeen capture metadata records have code0, waited/EOF true and kill_requested false. Status shape agrees with the actual legacy CLI emitter in `ordered_cli.rs`: object with contexts array, numeric phase/counters and string-or-null session_id. At30/32/33 both rows have phase3, nonempty string SID, send_next1/receive_next1 and pending/unacked0. Existing `old_zero_text_ready` accepts these exact sanitized predicates; its logic and workload remain unchanged. Locator import/setup commands precede these observations in the published flow. No earlier command/memory failure is demonstrated.

The published F sequence is `new_feasibility → start0/start1 → import1 → four readiness waits → stop1/stop0 → first Legacy::capture`. Legacy stop reconciliation skips current-only source reconciliation. `Legacy::capture` begins with `owned(node)?`; that original function canonicalizes state and checks the P-only namespace. The correct F path necessarily fails this predicate.

There is also an exact static error match, without exposing arbitrary errors: the source-defined line `Error: only this fresh S54 origin source\n` is41 bytes, SHA256 `8d291d890013e4a02939cf33c940ac5fd62d15c9e0af89b941c27b5996c3ad1b`. F05 outer stderr source-prefix metadata records exactly those41 bytes and that SHA. `cause-proof.json` binds the selected source constant, Git-original source snapshot, actual predicates/capture indices and retained prefix metadata. The matched error is the original namespace guard; no inference from a redacted arbitrary payload is needed.

This refines Q's intentionally limited public-only uncertainty. Original Q/P reports and pins remain unchanged. No memory failure, natural legacy exit, product handshake failure or cleanup-caused original error is inferred. Both legacy Kill outcomes and the observer abort are consequences/boundary observations, not substitutes for the demonstrated guard failure.

## Minimal change and guarantee

Two source/test paths:

- `migration_s54_source.rs`: `owned` obtains the attempt from the existing healthy admitted capture scope, resolves only the two current closed `Purpose` values, and validates canonical state against **that exact attempt's exact origin** (`p5-origin` or `f-origin`). It rejects the origin root itself, sibling/foreign/old attempts and escapes. There is no environment-supplied root or general F namespace allowance. Unadmitted access fails before source path access. Actual P constructor paths remain accepted; its former namespace-only bound is tightened to its admitted origin. Current F is accepted under the same bound. Existing source reads, lock/immutability/SID/session-key/receipt checks are unchanged.
- New `migration_s54_source_scope_tests.rs`: empty directory fixtures under this admin's synthetic workspace only; no runtime, private profile, database or key fixture. Three tests exercise exact P/F positives, cross-purpose/old F01–F04/invented attempt/foreign namespace/origin/root/missing/canonical escape refusals, and real `owned` refusal without an active capture before accessing an uncreated path. Two directly affected existing immutable Legacy tests retain original SID/all-three-session-key and source/receipt checks.

Production still uses the one-use admission/capture boundary; selecting a valid path alone grants no runtime or store capability. No purpose transition, deadlines, workload, successful-condition relaxation or product fix is introduced. F05 remains consumed/failed; the repaired source has not run against its private state.

## Reproducible before/after and actual returns

For the native baseline, the original P-only guard body was extracted without changing its predicate into the shared path helper, with the active-attempt parameter unused. The source snapshot and Git original are retained. The new positive regression first accepted P01, then failed F05 with the same static guard label: native test101, administrative child/owner1. This expected failure is preserved. The final repair uses the admitted attempt and exact origin; the same positive regression and four other selected tests passed. No entire F attempt or closed compiler/purpose/transcript matrix was rerun.

All compilation/execution used the existing mutable nonproduction `target/s54-diagnostics-source-checks`, compile-only then fixed EXE paths via `scripts/run-cargo-tests-stable.ps1`, verification profile, locked/offline/Jobs2/BelowNormal. Production F05 images/origins were untouched. Aggregate harnesses selected0 tests.

| Step | Actual tool return | Child/owner | Elapsed seconds | Result |
| --- | --- | --- | --- | --- |
| ps-01 | `bb0d3e` | 0/0 | 0.6438222 | bounded sanitized existing-log analysis |
| format-01 | `29886a` | 0/0 | 1.5489902 | baseline extraction/regression formatting |
| compile-01 | `58c22d` → `d6bb2c` | 0/0 | 37.4367303 | baseline compile-only |
| rust-01 | `c04386` | 1/1 | 3.8042582 | expected native101 P-pass/F-refusal |
| format-02 | `f6dd19` | 0/0 | 1.4629834 | scoped repair formatting |
| compile-02 | `5957d5` → `2668d9` | 0/0 | 36.6743196 | repaired compile-only |
| rust-02 | `5421dc` → `e81ed0` | 0/0 | 8.5962103 | five exact tests passed |
| seal-01 | `21ec90` → `a32355` | 0/0 | 11.1996906 | immutable input/source/scope/process checks |

Compile1170/1200/30 seconds; diagnostics/focused checks570/600/30. All eight receipts have verified actual handle/Job membership, waited child, bounded EOF and verified cleanup/after[]. Compile cleanup terminated owned residual61704 and61736; their executable identities were not captured or inferred. Other cleanup, including the expected failing baseline test, was nonforced. No failed evidence was overwritten.

Initial admin setup03ce43 exited1 before writing baseline/owner: it attempted to require an older historical navigation hash after root's legitimate later navigation edits. Preserved setup-01.py records this. Setup5f420e correctly excluded historical mutable navigation observations and pinned current navigation; all other immutable evidence stayed required. Static cause-proof generationb5cc20 exited0; its exact constant/retained metadata and all78 diagnostic input files were independently verified inside the bounded final seal. No production data or source repair was involved in that setup correction.

## Preservation and review boundary

Final seal verified692 immutable production/history/P/Q pins, all74 original command logs and four initial runtime logs unchanged,521 current source paths (one existing changed, one new;519 other original source files unchanged), current root navigation, baseline HEAD and empty index. Relevant process inventory succeeded with0 active images. No F06 or WholeP directories were created. Production images/origins, original F01–F05 run/public/admin artifacts, archived preimages and failed-state contents were not repaired/rebound/removed. The source-check cache is the only mutable binary cache used.

`candidate.json` contains exactly eight regular100644 paths/raw SHA256/normalized Git blobs: two source/test files, unchanged P preparation and Q failed-runtime reports/pins, this report and pins. `source-diff.patch` contains only the two source/test paths. Root navigation/task/review/acceptance files and unrelated changes are excluded. Original logs are referenced by pins, not copied. No commit/push, F06 transition/allocation/Prepare/activation/Run or WholeP work occurred. Stop for root review; this repair closes the demonstrated local incompatibility only and does not establish feasibility or whole-S5.4 acceptance.
