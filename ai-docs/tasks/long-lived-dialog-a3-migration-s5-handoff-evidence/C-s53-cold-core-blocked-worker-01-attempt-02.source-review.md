# C2-Q attempt02: bounded serialization repair

2026-10-31. **Dormant source only; STOP for review.** Implements the [accepted minimal repair](C-s53-cold-core-blocked-worker-01-attempt-01.failed-review.md). Attempt01 remains FAILED/consumed; no deadline coverage is inferred. No new contract or matrix.

|Source|Raw bytes|SHA256|
|---|---:|---|
|[Core](../../../scripts/migration-s53-cold-preservation.ps1)|80889|e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e|
|[Worker](C-s53-cold-core-blocked-worker-01-attempt-02.preflight.ps1)|69276|424e0ef4c03bb6c5a9950e042f1ffe6c3976de719fd520abd86300f6633c5c39|
|[Supervisor](C-s53-cold-core-blocked-worker-01-attempt-02.watchdog.ps1)|37619|5b1c7dba8d6fe13e92a8196b955ba4b848a1f105c974338f65a896541e7151d2|

Core repair delta +6/-2 relative to the root-frozen [80601B source](C-s53-cold-core-blocked-worker-01-attempt-01.core-source.ps1): append deadline-blocked-scan-r02 in both allowlists and add its fixed attempt02/blocked-scan mapping. Old deadline-blocked-scan/attempt01 mapping remains unchanged. Exact inverse reproduces that frozen source (SHA74f53161ddfb4862efb20249e7a6eeac11d9b1c71665a2755fff1fe6b907703d). Relative to published HEAD389ff0b863980733dc10bc21cdf43be43ca3bb72, the cumulative uncommitted Core delta also includes attempt01's reviewed selector. No production/algorithm/cap/arithmetic changes.

W/S are exact attempt01 copies after closed binding substitutions: fresh attempt02 prefix/root, selector, fixed event names Local\Kilogram.S53.C2Q01a02.ready/block, fresh premise type S53C2QStreamsR02, inventory/Core pins. Beyond those bindings, the **only supervisor behavior change** is adding -DateKind String to its witness ConvertFrom-Json call. Exact normalized comparisons confirm this; worker has no additional behavior change. Strict string/type/identity predicates are untouched, with no ToString coercion or relaxed oracle.

## Public-only serialization evidence

Builtins read the immutable [attempt01 witness](C-s53-cold-core-blocked-worker-01-attempt-01.witness.json),7258B/SHA7a6aab3c8a224beb37a7801c4c62a923b7261f89deeb5137c1c731f8f6444eb2. System.Text.Json independently extracted the raw expected value2026-10-31T03:55:12.2522441Z. ConvertFrom-Json -Depth12 -DateKind String preserves System.String and exact ordinal equality; serialize/reparse with the same option preserves the exact value and all50 fields.

Replacing only that JSON field's token with unquoted123 yields System.Int64 and fails the unchanged required-string/equality predicate. This is a narrow builtin public-data check, not a project Assert-Witness invocation, fixture run, scheduler observation or deadline test.

## Fresh model / unchanged safeguards

Inventory was regenerated in memory with the new fixed root: **3586B/SHA50a6edd1e45a0fb2d0e272d4d6ba10114e223e4b2ef42a81b4bb3f6b08c9799b**, different from attempt01. Independent parent/row path recount still gives36 interned strings/path7024, record12073840+32*384+5*160=12086928, retained32/peak33. Same-length root preserves geometry16dirs/13files/3690B/57stream checks. It does not justify reusing the old inventory digest.

Core/W/S Parser.ParseFile errors0, functions3/51/30, duplicate names0. All28 shared W/S functions remain identical. The independent start/guardian block is literally unchanged, as are strict oracles, forced=true outcome requirements, cutoff/kill/exit/reap/EOF clocks, cleanup reserve, MTA, witness closure/readiness sequence, honest historical32-owned Core/null final state and parent/worker resource distinctions. Five Core checked sites and arithmetic prerequisites remain unchanged.

All13 existing attempt01 files, including its W/S/contracts/notes/reports/allocation and the root-created Core snapshot, are retained and hash-compared before/after. The root's snapshot is reused, not duplicated. Deadline helper and existing failure artifacts are not edited.

Own delivery: new attempt02 W/S and this note plus the minimal current Core selector mapping. No runtime, project/helper function invocation, dot-source/Add-Type/compile, .tmp/event/private/network/product operation, allocation/ledger change, commit/push or S5.4. Work20/whole30/lane1/retry0 is still inactive for attempt02. **Callback and STOP for source review.**
