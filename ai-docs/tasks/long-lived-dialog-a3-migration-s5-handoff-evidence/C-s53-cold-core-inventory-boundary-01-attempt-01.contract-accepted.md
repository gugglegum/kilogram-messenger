# C2-P contract acceptance and arithmetic coverage decision

2026-10-31. Root accepts [the proposal](C-s53-cold-core-inventory-boundary-01-attempt-01.contract.md) for dormant source preparation only. Baseline753b5683c6e97120a210afaa33f5b5a90a5611c6, Core79709/ec3cd2190c4b67c201c02eeda6c68e5832e95ac114a2e570954e4a25a05f3bb9. Gatee523b6 verified source/contract pins, all five checked sites, bound calculations, streamed zero-control digest, HEAD and empty index.

## Arithmetic decision

For this pinned Core, root accepts source-bound arithmetic safety coverage for the five checked expressions, with the existing Number, Accounting.Add and aggregate remaining-cap guards. Constructor table charge<=12073840; aligned intern charge<=4294967328 even for maximum CLR string length; parser offsets/differences<=16777216; four full hash passes<=268435456. These bounds are below the respective integer maxima. Root verified closed cap selection, admitted bounded raw buffer, nonnegative row values, aggregate cap, one-use capture and copied observations.

This closes the synthetic arithmetic-safety obligation through static proof for these paths, not through an observed OverflowException. The arithmetic_overflow exception mapper is not runtime-tested or independently certified by this decision. No exception injection, cap increase, reflection mutation or new gate is needed. Changes to arithmetic, caps, admitted inputs, one-use behavior or observation ownership require revisiting the affected proof. The upcoming selector seam must preserve all five expressions and their prerequisites exactly.

## Inventory case

Accept one fixed boundary-inventory-size case with16777217 zero bytes/SHA2561003b1b5dc078189799a1216ce0f9fbcebb94e8b6b83c58c4b03345f07f94ced. The new selector must accept only that exact bytes/hash/files5/payload52 tuple; all old selectors retain their original preconditions and production behavior is unchanged. Permit only this closed factory tuple, not a general larger cap or caller-defined path.

Root verified ReadInventory rejects equal Spec/actual oversized length as inventory_binding/inventory/inventory before InventoryTake, inventory array allocation, Seek/Read/SHA/Parse. The actual file passes Acquire's separate64MiB metadata cap. Setup streams/validates the control using bounded chunks; it is raw size-boundary data, never a healthy JSON inventory. Prove factory returned and capture entered; factory ArgumentException, mismatch-only refusal or setup failure are not PASS.

Expected Core evidence: locks6, expected5/52, capturedfalse/captureQPC0/hash0/all digestsnull, peak_inventory_bytes0, peakowned27/retained26/record12083824, finalowned0/inputrelease/knowncleanup, no output/receipts. Inventory payload non-read follows the reviewed control flow plus observed exact stage/cap accounting; do not claim a newly instrumented syscall count.

## Authorized next step

Prepare only new W/S and source-review note plus minimal closed selector/factory edits in Core. Reuse accepted checks and guardian; no generic large-file framework. Independently recount fixed-path arena and setup geometry before runtime. Native setup reads/hash of the synthetic control are distinct from Core's zero inventory read.

No dot-source/Add-Type/compile/runtime, synthetic filesystem/ACL operations, private access, active allocation, budget/ledger change, commit/push or S5.4 is authorized at this source phase. Prospective20/30 lane1/retry0 stays inactive.

Finite synthetic remainder after this case: blocked-worker deadline/final-return verification and producer-size positive with required metrics/receipts. Accepted C2-C/I/J/M/L/N/O and small-positive scopes remain accepted. Existing frozen launcher integration issue and later private/typed prerequisites remain separate.
