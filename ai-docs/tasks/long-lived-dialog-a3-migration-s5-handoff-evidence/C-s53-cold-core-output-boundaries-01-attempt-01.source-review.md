# S5.3 C2-M output boundaries: source review

2026-10-31. **Source/static only**, Sol High/no Fast. Baseline: published/independently verified C2-L `b24b31533b1388bf5c0809aa6b8aed59d9545567`. Two outstanding [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md) obligations under [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and the [workflow](../../verification-workflow.md): aggregate output cap and fresh E collision. Accepted tests are not rerun.

Only [Core](../../../scripts/migration-s53-cold-preservation.ps1), new [worker](C-s53-cold-core-output-boundaries-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-output-boundaries-01-attempt-01.watchdog.ps1) and this note belong to this package. Family `C-s53-cold-core-output-boundaries-01-attempt-01`; fixed cases `output-cap` and `directory-collision`. Core diff **+8/-2** adds two closed selectors/path bindings and maps boundary-output-cap to the existing cap-output/Output512. The other selector is boundary-directory-collision. Inverse substitutions restore HEAD exactly. No production/default constant, Receipt.Record/WriteReceipts or receipt-close algorithm changes.

| Source | Raw bytes | SHA256 |
| --- | ---: | --- |
| migration-s53-cold-preservation.ps1 | 78724 | `bcb6776ec8e8e49008d3cff867e2fc6cdd917d4257b3c7560e999a8ac14760f4` |
| C-s53-cold-core-output-boundaries-01-attempt-01.preflight.ps1 | 110948 | `98e58c85b3fce63f8e604a6b1d340249b5ea2954295da7b007840394cbe87438` |
| C-s53-cold-core-output-boundaries-01-attempt-01.watchdog.ps1 | 37370 | `70f9e79c51b9db6dcbaec218e2044370d334f684d22bdeeb1d9eb5917bc49664` |

## Healthy prerequisites and separate refusal profiles

Each independent case starts with five producer-shaped rows/52B mirrored payload, six empty locks and safe-empty-S. Both complete CaptureAndCompare, all four hashes/208B and H=`54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`, with input handles released **before** any E action. Historical provenance/maps remain unchanged; only fixed family/case headers differ.

**Output-cap is a Core refusal.** Normal restricted owner creates fresh private E and transfers its directory; first owner.private.json is created synchronously/share0/CREATE_NEW and transferred. Its first Record fails aggregate OutputTake **before charging or any WriteBytes**, with exact output_cap/receipts/receipts, bounded_refusal/native0 and the reviewed three-node semantic exception chain. Per-file cap65536 does not fire first.

This does not depend on timestamps: JSON containing only the actual decision_sha256/core_sha256/source/preserved/output members occupies **546 ASCII bytes**. These five mandatory members alone exceed512. A maximal-width model of the full fixed17-field/six-lock schema is **3889 bytes including LF**, using19-digit positive Int64 timestamps, UInt32 maxima, signed FILETIME minima and16-digit file IDs. Thus 546<=actual record<=3889<65536, well within existing record scratch. Root braces already counted in the subset cannot make the complete object shorter. The model uses the actual fixed ASCII strings and JSON escaping, never a random runtime timestamp.

Receipt.Record checks per-file size, then Accounting.OutputTake checks n>cap-value before incrementing Output. The refusal is outside the write-error catch. Receipt.Dispose still closes its fresh empty handle; Complete was not reached, no digest is published. WriteReceipts finally closes the output directory. Expected completed=true, receipts_completed=false, receipt_write_unknown=false, charged/knownwritten0, cleanup_unknown=false/cleanup_native_error0/remaining0, full capture/hash facts retained. Bound Core ownership1 becomes0 on refusal and stays0 after Dispose; peak35. A closed-path verification reopens only owner.private.json read-only, checks its original ID/path/type/private ACL/size0, reads zero-byte EOF and explicitly closes it. No successful WriteReceipts result is accepted.

**Directory-collision is an output-owner setup refusal, not a Core error.** After successful capture, a separate fixture owner creates its fixed proof directory with the same restricted security descriptor, holds its directory and six ancestors, and pins its identity/non-access stamp and exact ACL. A second real output owner attempts unchanged CreateDirectoryW at that same literal. It must return false/error183; its DirectoryCreated=false, no directory transfer/factory call/receipt. The fixture-preexisting flag is separately true. No BindOwnedOutput or WriteReceipts call occurs; Core failure/cause/native fields remain null/null/0, ownership0 and only capture phases.

[CreateDirectoryW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectoryw) documents ERROR_ALREADY_EXISTS for an existing directory; [Windows error codes](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-) defines it as183. The exact result must agree with the immediately captured helper return/error and two-node MethodInvocationException → Win32Exception(183), with no deeper cause. Other permission/sharing/path/reparse errors fail. The source-derived HResults are -2146233087 and -2147467259: [.NET10 Win32Exception](https://raw.githubusercontent.com/dotnet/runtime/v10.0.0/src/libraries/System.Private.CoreLib/src/System/ComponentModel/Win32Exception.cs) delegates to [ExternalException](https://raw.githubusercontent.com/dotnet/runtime/v10.0.0/src/libraries/System.Private.CoreLib/src/System/Runtime/InteropServices/ExternalException.cs), whose message constructor sets E_FAIL. No localized error message matching.

The fixture rechecks the same retained directory, full checked non-access stamp, exact ACL and ancestry after collision and after Core.Dispose, then closes its own handles. No ACL change, deletion, rename, truncation or cleanup repair. Zero receipt creation/writes follows from fresh directory creation, zero creator calls/masks/transfers and no Core output entrypoint; **no output namespace/ADS enumeration or continuous-absence proof is claimed**. Retained identity/ACL checks are real later checks, not a reused before snapshot.

## Ownership, clocks and observability

Fixture-only BoundaryOwner is embedded in the worker, with exactly two case-name bindings and no arbitrary path/command. It derives from frozen [BaselineSmallOwner](../../../scripts/migration-s53-cold-output.ps1); original source remains15195B/`81f625dd3334c5ce6ba5d7efb36d54404ccfefd1bbe03849312121a241da4dea`. Keep its private security, checked ancestors/current-handle try/finally, no-follow/CreateNew, sole transfer, ACL/path/type/link checks and explicit native close. The new creator accepts only first owner.private.json. CloseOwned only adds counters around the original native result/accounting; PrepareDirectory adds direct call/result/error/QPC instrumentation without moving immediate DirectoryCreated ownership. **All protection/weak-token/native-close fault/reclaim code is absent** from this helper.

Output-cap helper ownership: prepared7 → directory transferred6 → fault6 → zero after one empty-file verification close and six ancestry closes (seven explicit helper native closes; Core's two successful closes inferred separately). Collision fixture retains7 while the failing output owner holds6 ancestry handles; combined13, then0 after separate6+7 successful closes. All current native handles stay in existing local try/finally until transfer; no new managed enumerator/collector or hidden output writer is added.

Worker and parent retain/assert all **38 Core fields** in four cap snapshots (capture/bound/fault/disposed) and three collision snapshots (capture/refusal/disposed). Cap gets13 hook phases, collision11, both10 acquired-file hooks. Output binding, write/failure, collision, dispose/verification QPCs share the absolute deadline; parent validates their ordering and separately asserts absent entrypoints/zero clocks. Core observations remain identical after later fixture cleanup; no intentional cleanup_unknown exception is allowed here.

Eight helper snapshots, each exact **28 fields**, cover cap prepared/bound/fault/final and collision failing-owner fault/final plus retained-fixture before/final. Facts include directory creation ownership, native CreateDirectory call/result/error, creator/mask/transfers, native close attempts/successes, cleanup/current ownership and verification identity/ACL/EOF, with four exact clock fields. Parent requires the precise case profile, separately known final helper/controller cleanup, child exit0/EOF/reap and normal guardian completion. Unrelated errors/timeout/unknown cleanup never count as observed. Whole-attempt verdict remains not_established.

## Resource and static model

| Quantity | output-cap | directory-collision |
| --- | ---: | ---: |
| Inventory raw bytes |3590|3626|
| Inventory SHA256 |d6a5d514eaa6632c3cf7a925270296eb7eed703c1d54f9fe8af0fdeddccbe83d|4de547203e84938743b6f549c4cad23fc245148a7a2f6e23bbe1a7b9dcf69212|
| Interned strings / path bytes |37 / 7112|37 / 7488|
| Core record / scratch bytes |12087312 / 524288|12087312 / 524288|
| Core peak owned handles |35|35|
| New proof directory / receipts / receipt bytes |1 / 1 / 0|1 fixture-owned / 0 / 0|

Core acquisition26 fixed+six ordinary payload+safe extra=33; record precharge12073840+33×384+5×160=12087312. Paths sum align8(32+2×UTF16 length) over26 fixed paths/five rows/five parents/safe-empty; peak35 is retained33 plus find/metadata. Output runs after input release and does not raise it. These are accounting values, not process working-set measurements. New helper scratch is fixed StringBuilder32768, <=16 ancestors and one-byte EOF buffer; Core arenas unchanged.

Aggregate controlled geometry: **33 directories,27 files,113 input stream checks,7424 default-stream bytes**. Family guard5 plus two case guards15 each gives35 returned/closed, case guards relinquished before Core. Payload/input ADS premises remain strict; fresh output default-stream creation is separate from those113 checks. Empty read/metadata checks are not ADS enumeration.

Source gates: AST0/Core3 definitions/worker53/supervisor30;27 common functions exact, all38 Core/28 helper keys covered, no undefined Spec members. Embedded C# lexical delimiter balance and11 unique usings; two fresh helper types/five fresh-type gates/three initializer calls. Public raw data model validates both inventories/three tuple maps/5row52B/H, Core inverse and frozen output/RootCopies/deadline pins. Default diff and raw whitespace checks accompany callback. Compilation/Windows results are unverified.

No .tmp fixture query/create/read, Add-Type/compile/native/Rust/product network runtime, private/keys/DB/original access, allocation/reservation, ledger change, commit/push or old evidence edit occurred. Proposal only: **work20/whole30, lane1/retry0**, unchanged source131072/report65536each/196608total caps and reviewed guardian/job/Pump/EOF/reap. Source sizes fit; no measured reason for larger bounds yet.

Accounting remains closed S5.3 C1579.9784909/F1562.2882567/P0; combined C2167.4271018/F6278.6084056/P0; ceilings1620/1642/0; active0/inactive C40.0215091/F79.7117433/P0. No private grant; whole S5.3/G1/S5.4 remain open. Callback exact4 pins/diff/profiles/models, then **STOP for source review** before any runtime or publication.
