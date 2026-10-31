# C2-N accepted: native read errors

Root acceptance: 2026-10-31. Baseline: d0e58c93e7287ca9b16c1eb246a6679ce86038da.
Source review root1736e7 and allocation root4e88ca remain applicable. Root actual validation independently checked 380 assertions across six 38-field Core snapshots, three 36-field lock-owner snapshots, exception chains, QPC ordering, ownership, and the actual app command item. Live gate afa4b0 verified all 13 input/report raw byte counts and SHA256 pins, unchanged baseline, and empty index.

## Actual execution

Executor turn: 01a18fb0-f6d9-7540-8312-28e5d10b888d.
Actual app item: exec-68f1f885-1071-425d-97b9-e3e8e150c5ee; native chunk 6f713f, exit 0.
One native invocation, no continuation or retry. Native duration 4.1588097 seconds; request-to-complete 5.725 seconds. Worker 3.3940705 and supervisor 3.9039239 overlap and are not added.
Child 46020 exited 0, was reaped, both output streams reached EOF, managed inputs and stdio were disposed. No forced termination, output cap, cleanup uncertainty, or remaining owned children.

## Accepted observations and limits

- read-locked: a compatible read-only helper held an exclusive one-byte range lock through Core failure and disposal. Core refused with input_busy/hash_s_before/s, native 33, Win32Exception HResult -2147467259. Ten inputs acquired; zero complete hash passes and 23 bytes read; historical digest retained and all observed tuple digests null. Core peak 35 and final owned handles zero. Helper verified object/stamp/ACL, unlocked and closed exactly once; final ownership zero and cleanup known.
- streams-unsupported: documented information level 0 with fixture-only reserved flags 1 produced native 87, mapped to streams_unsupported/capture_s/s. Two acquired inputs, zero hashes/capture timestamp, peak 29 handles and final zero. This checks actual invalid-parameter error mapping and cleanup; it does not establish behavior on a genuinely unsupported filesystem.
- Both cases preserved setup ownership accounting: 39 returned file objects closed and 30 enumeration objects disposed per case; aggregate directory guards 35/35 closed. No output owner, receipts, proof, private dataset access, or network access.
- The narrow fixture predicate and approved default native-call behavior remain unchanged. Source and reports match the reviewed pins.

## Accounting and publication

C30 CLOSED once: charge 5.725; release 24.275.
S5.3 C1593.4174909/F1562.2882567/P0; combined C2180.8661018/F6278.6084056/P0.
Ceilings C1620/F1642/P0; active reservation zero; inactive C26.5825091/F79.7117433/P0.
No new private access grant. Whole S5.3 remains open; S5.4 is not activated.

Publication scope is exactly ten files: scripts/migration-s53-cold-preservation.ps1 and this prefix's preflight.ps1, watchdog.ps1, source-review.md, root-allocation.json, started.json, cases.json, watchdog.json, run.json, accepted.md. Navigation edits and unrelated worktree changes are excluded.
