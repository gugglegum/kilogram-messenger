# Cold-A numeric validation: source-only review

2026-10-31. Root authorized this minimal repair after confirming the production P2 reread blocker. Baseline commit `3ac9e55f08763111897ef7290f5f0f8ec469d9ad`. **No regression execution or private activation in this slice.**

## Focused change

[Launcher](../../../scripts/invoke-migration-s53-cold-preservation.ps1):280–281 changes only the output and parent_guards numeric loops. Each value must be **Int32 or Int64 and within Int32.MinValue..Int32.MaxValue**. This preserves the old signed Int32 value domain, including negative counters, while accepting its JSON Int64 representation. Strings, doubles, booleans, null, wider integers and coercion remain excluded.

Launcher:294 changes only stdout_chars/stderr_chars type predicates to Int32-or-Int64; both existing **0..4096** bounds remain. This also covers the supervisor same-class roundtrip inconsistency, although production Read-Report currently excludes supervisor.

Exactly **3 added / 3 removed lines**. Reversing these three replacements reconstructs the committed launcher byte-for-byte in memory; independent disk SHA matches the new text. No other validation, path, ownership, clock, source helper, Core/output or accepted C2-T report was changed.

| Source | Bytes | SHA256 |
| --- | ---: | --- |
| launcher | 71513 | 8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557 |
| [numeric regression](P2-s53-exhausted-cold-a-01.numeric-regression.ps1) | 7823 | 5d6e6853f4d140941d872abfb8e5c1a5237648218be99d628bb16fe3c086d38b |

## Dormant regression source

The standalone 126-line script reads only the fixed public launcher, pins its full raw bytes/SHA and checks the fixed bundled shell path. It parses source AST, selects the actual two ForEachStatementAst loops from Validate-Report and actual Need command for public_stdio; exact extracted-text SHA pins prevent selector drift. Member names come from those actual loop conditions. It defines only the actual extracted **Stop-Cold, Need, Value** dependencies, each separately pinned. It never imports/calls the launcher entrypoint, Validate-Report as a whole, or any production filesystem/process helper.

Planned coverage (not observed results):

- Every one of 12 output/guard numeric members: Int32 and Int64 at signed Int32 min/max, -1/0/1; reject Int64 outside Int32 (including Int64 min/max), strings, integral/fractional doubles, booleans and null.
- Both stdio members: Int32/Int64 at 0/4096; reject negative, 4097, Int64 extremes, strings, doubles, booleans and null.
- Two full in-memory public-shaped counter roundtrips using **ConvertFrom-Json -Depth12 -DateKind String**, covering both signed endpoints for every output/guard member and 0/4096 for stdio. Assert decoded Int64 and execute the actual extracted statements.
- Expected coverage totals: **134 accepts, 158 exact refusals, 28 observed Int64 values**. Rejections must carry the actual InvalidOperationException/HResult -2146233079 and exact cold_launch_code public_output/public_guard/public_stdio; setup/unexpected exceptions are failures, not PASS.

No Add-Type/semantic compilation, native helper, process spawn/network, private path/fixture tree or file output is used by the regression body. Its future output is one aggregate JSON to stdout. This is **numeric-validation regression only**, not full production Parent/job/claim/private end-to-end or Cold-A preservation evidence.

## Static checks and current boundary

Both PowerShell files parse with **0 errors**; source diff whitespace check passes. Source AST extraction was inspected without invoking extracted statements/dependencies. No test cases or JSON roundtrip from this regression have run. Raw test file is strict UTF8/no BOM/LF; no runtime/allocation/ledger/navigation/commit/push.

The prior three Cold-A D1/plan/preparation-review documents remain untouched and inert, referring to old launcher71277B/12661c746bfad302d5dabf0acdb5ea621005f5650120300a5e971d7507e6d976. They are **invalid for use**. Their prospective active hash7f48032a87ab16f8102959d9f9e93e8f36e82dac4b8b4e2afe94d6a4d6b838bd must not be activated. Repinning waits for root review, any separately authorized regression, acceptance and publication of the new launcher. Existing C2-T evidence remains historical, not regenerated. No private grant is implied; whole S5.3 remains open.

Return source/test/review pins to root and **STOP at source review**.
