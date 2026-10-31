# C2-R contract acceptance and R1 source task

2026-10-31. Root accepts contract15584B/SHAea1402b1a83bf152565d6eb07c3083190586c14a6d7590f23e2624f7ea02fe6d on baseline0cbc322fe3ae06632af7ea4c1538f19b060d22ac, subject to ordinary source proof of the declared premises. No runtime activation.

Rootea625b independently regenerated all6956 rows (6954 nonempty +2 locks), exact three-array producer schema/provenance from actual producer and its literal rule,3465145B inventory/c40ffdedd028be593c08ded6b05daba44abe0d1a9e1dac86cb6d78632cbf3843, H2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf. Verified physical13915 files/94659943B, four hash passes182389596B,26 fixed paths/6988 interned/path563128, record18537840 and receipt aggregate arithmetic. This was public in-memory arithmetic, no fixture/native execution.

Source reads confirmed production limits and producer includes two zero-length lock rows. Core capture, BindOwnedOutput, accounting and receipt semantics support the chosen lifecycle; exact peak13938 and owner/result fixed-record fit still need implementation/source proof, not runtime guesses. Native-dependent receipt lengths/digests remain measurements. Existing Q negatives are not repeated.

Official Process.HandleCount and PeakWorkingSet64 documentation supports current refreshed OS handles versus process-lifetime memory high-water; sampling gaps/exit tail limitations stay explicit. No post-exit query is required; a live observer failure cannot be converted to success. Any exit/sample race must be handled explicitly at R2 source review, not by catch-all PASS. Ref: https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.handlecount?view=net-10.0 and https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.peakworkingset64?view=net-10.0 .

Prospective420/480 is accepted for implementation wiring only: setup-heavy filesystem work needs reserve, and this is no speed estimate. No ceiling increase/reservation now; ledger remains spentC1634.6434909/F1562.2882567/P0; combinedC2222.0921018/F6278.6084056/P0; ceilings1670/1642/0; active0/inactiveC35.3565091/F79.7117433/P0. Root allocates only after complete source review.

## R1: bounded fixture and independent verifier helper only

Implement one dormant synthetic helper at this attempt prefix .fixture-support.ps1 (or one equally explicit single helper source). Scope: deterministic streaming generation/oracle, current-owner fixture setup/readback and two bounded healthy traversals, streamed strict inventory/map/provenance validation, streamed four-receipt independent verification, exact resource counters and cleanup. Reuse accepted actual native ownership/ACL/stream primitives; do not refactor product algorithms.

Bound metadata/queues/current resources before allocation, clear disposed references, preserve native cause/partial evidence, no whole inventory DOM or whole comparison receipt collection. A bounded single-record DOM <=32768B is allowed; all three inventory arrays require bounded streaming as contracted. Use exact types and ordinal identity; public output aggregate-only. Parameter APIs should expose only fixed closed synthetic scope and explicit deadline/check callbacks, no ambient production/private root or arbitrary file access surface.

Do not implement sampler/worker/supervisor yet; do not edit Core/output helper/current W/S. Export a small explicit interface for R2, with no top-level execution/filesystem access. Source-review should show API/lifecycle, allocation limits, ledger reconciliation13915/41745, independently calculated expected pins and receipt bounds, source-only AST/C# text checks and potential unresolved assumptions. Prove owner/result fixed-schema record fits32768 from current writer field bounds; if not feasible here, identify exact remaining proof for R2 rather than claiming done.

No runtime/fixture/events/.tmp/private reads, dot-source/import/Add-Type/compile/project/native/Rust/network execution, grants/ledger/navigation changes, commit/push or S5.4. Allowed public source/evidence reads, static in-memory calculations, edit only this fresh helper plus concise .r1-source-review.md. No tests that simply mirror implementation; no broad new matrix. Callback and STOP for focused R1 review.

R2 follows accepted R1: minimal closed Core/output mappings, worker/supervisor integration,100ms observer, same-clock420/480 lifecycle and honest success/reporting. R2 source review then one allocated runtime, root actual acceptance and commit/push. These are two reviewable parts of C2-R, not new project stages.
