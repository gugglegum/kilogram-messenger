# Accepted C2-G filesystem-alias refusals

2026-10-31. Root accepts exactly the two synthetic filesystem-alias cases. Whole cold preservation, private access, G1 and S5.3 remain open.

The [source review](C-s53-cold-core-filesystem-aliases-01-attempt-01.source-review.md) was independently checked: Core differs from published f9a50db03aabf2330c9101c9eeacfe120ed55731 only in three closed fixture-binding substitutions (153fc1). Root reproduced path/resource expectations and inventory bytes/hashes (793183). Twelve shared oracle bodies and the existing supervisor/pipe lifecycle matched (d4dadf). One review correction removed the junction from stream-checked counts without adding a scenario; exact inverse and final AST checks passed (761dd2/5e791a).

One native invocation, executor turn 01a18efa-ae26-72f1-a601-56f8fc496efc, app item exec-f2ad8a7a-3241-47b7-8a22-173da5dea30a, returned exit0. Root independently matched the actual complete app command/output/status/duration to [run.json](C-s53-cold-core-filesystem-aliases-01-attempt-01.run.json). Chunk f30657; native 4.6077404s. Request interval 2026-10-30T22:43:09.687Z–22:43:17.380Z = 7.693s; worker 3.7134334s and supervisor 4.2892135s overlap and are not added.

The [worker](C-s53-cold-core-filesystem-aliases-01-attempt-01.cases.json) and [supervisor](C-s53-cold-core-filesystem-aliases-01-attempt-01.watchdog.json) both observed:
- Hard link: file_alias/capture_s/s/native0/bounded_refusal; locks6/acquired2, hash0/capture0; pre owned26/peak27, final remaining0/peak28. Historical tuple retained, four pass digests null.
- Junction: reparse/locks/s_one_lock/native0/bounded_refusal; locks2/acquired0, hash0/capture0, final remaining0/peak14; all five tuple digests null.

Root checked all 38 fields in each of four Core observations, semantic exception chains, reached phases/times, setup/guard ownership and both 35-field alias-helper observations. Both helpers returned/closed four file owners; hard-link find4/close4/read24; junction move1/set1/get2. Setup file-result owners38/39 and enumerators30 each closed; case directory guards15/16 closed before factory. Aggregate guard ledger36/36, remaining0. Child32860 exited0/reaped, both EOF, empty pipes, stdio/managed inputs disposed, owned children0, no forced/cap/unknown result.

Geometry32 directory names/27 file names/114 stream checks/7454 logical default-stream bytes. Junction type/target was checked separately; its own stream set was not observed. Zero named-stream bytes applies only to the observed/created stream scope. Normal closure is not native false-close coverage; continuous namespace absence is not claimed.

Strict JSON/pin gate187192 verified all four reports, nine exact public inputs before/after and live, and native-report total21457 bytes. Source was unchanged through the run. No original/private dataset, key, database, network or product runtime access; no retry.

C30 CLOSED once: charge7.693, release22.307. S5.3 actual C1542.5644909/F1562.2882567/P0; combined C2130.0131018/F6278.6084056/P0. Ceilings1570/1642/0; active0; inactive C27.4355091/F79.7117433/P0. The allocation remains an immutable historical activation record.

Publication acceptance: only the changed Core and this attempt's preflight, watchdog, source-review, allocation, started/cases/watchdog reports, run collector and this acceptance note (10 files). Parent f9a50db03aabf2330c9101c9eeacfe120ed55731; unrelated worktree/navigation/product changes excluded. Root verifies the exact committed diff and remote after publication.
