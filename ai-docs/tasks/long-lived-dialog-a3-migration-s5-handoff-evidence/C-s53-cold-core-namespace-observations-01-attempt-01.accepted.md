# Accepted C2-H namespace observations

2026-10-31. Root accepts exactly the two synthetic namespace cases. Whole cold preservation, private access, G1 and S5.3 remain open.

[Source](C-s53-cold-core-namespace-observations-01-attempt-01.source-review.md) was accepted without corrections. Root checked the three closed Core fixture substitutions, inventory/path/resource model (3c7d81), AST, twelve shared oracles and unchanged native supervisor/pipe lifecycle (29ea8a). The one-use mutation creates only its fixed owned synthetic file after both initial scans. The writer and independent reader close before hashing continues; setup/hook/cleanup errors cannot qualify as the expected Core refusal.

One invocation: executor turn01a18f14-be6f-7ec3-ba44-e4fc4a552418, app item exec-fd923229-c3d0-4c49-a217-0486b465b654, chunkab2c4b, exit0, complete output. Root matched the actual app command/status/output/duration to [run.json](C-s53-cold-core-namespace-observations-01-attempt-01.run.json). Native4.8461226s; request interval2026-10-30T23:11:23.271Z–23:11:30.063Z =6.792s. Worker3.9483326s and supervisor4.513726s overlap and are not added.

Both [worker](C-s53-cold-core-namespace-observations-01-attempt-01.cases.json) and [supervisor](C-s53-cold-core-namespace-observations-01-attempt-01.watchdog.json) observed:
- Extra backup directory: directory_extra/scan_before/bkp/native0/bounded_refusal. Locks6/acquired10, captured before scanning, hash0/bytes0, historical H plus four null pass digests.
- Persistent extra state file: entry_set_changed/scan_after/s/native0/bounded_refusal. Locks6/acquired10, four completed hash passes/208 bytes/all five H digests. One create and two verified reads; writer/readers closed. QPC evidence places creation after the first scans, closure before the next hash phase and the final read after scan_after.

Root independently checked all38 fields of each of four Core observations, semantic exception chains, reached phases, capture timestamps, the14-field mutation record and cleanup. Case file-result owners39/42 and enumerators31/30 closed; directory guards16/15 closed before factory. Aggregate guard ledger36/36/remaining0. Geometry32 directory names/27 file names/116 stream checks/7525 logical default-stream bytes. Child51456 exited0/reaped, both EOF, empty pipes, managed/stdio disposed; no forced/cap/unknown outcome or remaining owned children.

Gate45ba1c checked four report pins, nine exact public inputs before/after/live and native-report total22248 bytes. Four JSON files were strictly parsed. No source change/retry, original/private state, keys, database, network or product runtime access. Continuous namespace absence and native false-close behavior remain unproved.

C30 CLOSED once: charge6.792, release23.208. S5.3 actual C1549.3564909/F1562.2882567/P0; combined C2136.8051018/F6278.6084056/P0. Ceilings1580/1642/0; active0; inactive C30.6435091/F79.7117433/P0. Historical allocation stays immutable.

Publication scope: changed Core plus this attempt's preflight/watchdog/source-review/allocation/started/cases/watchdog/run/accepted files (10 total). Parent df471f7a6660b03cc381ffaef14d4c727f369c5c; unrelated navigation/product/worktree changes excluded. Root verifies committed blobs and remote after publication.
