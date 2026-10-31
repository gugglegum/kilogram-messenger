# S5.3 C2-L receipt native-close source review

2026-10-31. **Source/static only**, Sol XHigh/no Fast. Root's C2-L dispatch follows published C2-K `d439740a8ca74b6ce27670366b7370d35b577b1b`. This prepares the isolated R44-4 regression in [r45 §4](../long-lived-dialog-a3-migration-s5-handoff-cold-core-readiness-r45.md), accepted by [root review](S5.3-r45-cold-core.root-review.json), under [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and the [current workflow](../../verification-workflow.md).

Scope: [Core](../../../scripts/migration-s53-cold-preservation.ps1), new [worker](C-s53-cold-core-receipt-close-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-receipt-close-01-attempt-01.watchdog.ps1) and this note. Core adds only C#/PowerShell allowlist entries and one fixed path/attempt branch: selector `receipt-native-close`, family `C-s53-cold-core-receipt-close-01-attempt-01`, case `receipt-close`. Inverse substitution restores HEAD exactly; production/default caps and receipt algorithms stay unchanged.

| Source | Raw bytes | SHA256 |
| --- | ---: | --- |
| Core | 78215 | `7fe9ad4f60fe8f4e5b2ae6b191db183968e19084e0f4e39f8f23d87e2ba759ca` |
| Worker | 109758 | `0464baa3a90bc959997740270730e61110af52dcddd3e1c31f1bacdc62286d02` |
| Supervisor | 34523 | `54b9e2742dc5a24603c092f756cf600eea2d9f98b2cfab53f7af2445b2877d28` |

## Ownership and the real native failure

Fixture-only `ReceiptCloseOwner` is embedded in the worker alongside the renamed strict stream checker. Original [BaselineSmallOwner](../../../scripts/migration-s53-cold-output.ps1) remains frozen:15195B/`81f625dd3334c5ce6ba5d7efb36d54404ccfefd1bbe03849312121a241da4dea`. Fourteen method bodies are reused exactly: Work, CloseOwned, Check, OpenDirectory, Acl, AclSnapshot, AssertPrivate, Security, FreeDescriptor, Role, PrepareDirectory, TakeDirectory, VerifyExactAcl, RecheckAncestry. Restricted ACL/CreateNew, synchronous read/write, share0, final-path/type/single-link and noninheritance checks remain. Creator accepts only the first owner.private.json request at the fixed Proof path.

1. Factory alone owns the fresh SafeFileHandle. After empty-file/object/private-ACL checks, SetHandleInformation(mask2,flags2) and GetHandleInformation(flags exactly2), it repeats object/ACL checks and returns the handle. Its owning reference/count is dropped. Only a nonowning raw token, original identity and long weak wrapper observation remain.
2. Core is then the sole closer. PROTECT_FROM_CLOSE means CloseHandle will not close this handle: [SetHandleInformation](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-sethandleinformation), [GetHandleInformation](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-gethandleinformation). No mocked result, invalid handle or double-close is used. Unchanged CloseFile saves result/error, invalidates and disposes the wrapper on either result; false retains its budget slot. Receipt.Close omits the digest even after Complete.
3. Driver validates the exact fault/snapshots and returns from Core.Dispose **before** its sole reclaim callsite. Reclaim requires the old wrapper IsClosed, or a null long weak observation after finalization/collection, plus flags exactly2 and the same live file ID/path/type/private ACL/expected length. The protected lifetime prevents recycling within this closed fixture. A temporary checking view has ownsHandle=false. Core's invalidation/disposal and Receipt's closed guard prevent later owning/finalizer retry.
4. Fixture explicitly reclaims responsibility, reads the same handle with bounded partial-read/EOF checks, verifies owner JSON, clears only protection (mask2/flags0), verifies flags0 and attempts one cleanup CloseHandle. It then disposes six ancestry handles. Any unverified transfer, wrapper still open, flag/identity drift or unsuccessful cleanup fails. Dispose leaves unreclaimed transferred tokens unknown; it never blindly unprotects/closes them. Failed cleanup is not retried.

Core's one native false close is **inferred from unchanged control flow and the protected-handle premise**, not direct Core call instrumentation. Helper counters directly measure their named set/query/reclaim/cleanup callsites, not every metadata query. Specific GetLastError is not documented here: dynamic Core cleanup_native_error is retained separately from semantic native_error0.

## Expected profile, without concealing unknown Core cleanup

Healthy premises: five producer-shaped rows/52B, mirrored payloads, six empty locks, safe-empty-S. Inventory **3586B including LF / `299750f5179b2e2905175b53212c454cd3a0602f5654f30cfc2bcb2533d01112`**; three matching tuple maps, derived-empty source-lock provenance, actual-byte destination provenance. Historical/all four tuple digests: `54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`.

CaptureAndCompare succeeds first: four hashes/208B, all inputs released before output. WriteOwner completes JSON+LF, flush and length verification, then protected close fails. The next Receipt constructor's Work refuses **before HandleTake or a second creator call**. Expected `cleanup_error/receipts/receipts`, bounded_refusal/native_error0, three-node MethodInvocationException → normalized InvalidOperationException → marked InvalidOperationException chain; semantic HRESULT -2146233079. WriteReceipts finally releases its output-directory slot.

| Core fact | Capture returned | Output bound | Fault returned | Dispose returned |
| --- | --- | --- | --- | --- |
| completed / expected_set_captured / observed_sets_equal / inputs_released | all true | same | same | same |
| receipts_completed / receipt_write_unknown | false / false | same | same | same |
| cleanup_unknown / remaining_owned_handles | false / 0 | false / 1 | true / 1 | true / 1 |
| charged / known-written receipt bytes | 0 / 0 | 0 / 0 | N / N | N / N |
| receipt digests | empty | empty | empty | empty |
| cleanup_native_error | 0 | 0 | dynamic | unchanged |
| phase keys beyond allocation+11 capture phases | none | output_binding | output_binding,receipts | unchanged |

Four complete **38-field Core snapshots** are retained. After fixture cleanup Core's public observation must remain byte-identical to its post-Dispose snapshot: historical unknown/slot1 are never reset. Capture/phase/lifecycle QPCs use the same absolute deadline and checked ordering; acquired-file hooks10/phase events13.

Five **30-field helper snapshots** cover prepared/bound/fault/Core-disposed/final. Owned counts **7/6/6/6/0** mean six ancestors+directory, directory transfer, nonowning receipt observation, reclaim(+1)/successful close(-1), ancestry disposal(-6). Final facts require one creation/transfer, one protect set/dedicated verification, one reclaim verification, wrapper closed/finalized, owner JSON verified, one clear/dedicated verification, one cleanup close/error0, disposed/known-zero ownership. Raw tokens are never emitted.

N is dynamic because of timestamps/IDs/stamps. Core charged=known-written N must equal the length read from the same reclaimed file, 0<N<=65536, with EOF. Verification requires exact17 root fields, six exact6-field lock objects, exact9-field stamps, bindings/provenance, one JSON record, exactly one terminal LF and no interior LF/CR. No digest is manufactured after failed close.

Parent accepts only this exact deliberate-fault profile **plus separately known fixture/controller/worker cleanup**, child exit0, EOF/reap. Earlier/unrelated refusal, timeout or any other unknown fails. Observed means this isolated injected native-close refusal; whole_attempt_verdict stays not_established.

## Models, static checks and remaining boundary

- Core precharge:60000×64+40126×72+8000×104+20000×224+32768 = **12073840**. Fixed acquisitions26+ordinary payload6+safe extra1 =33; +33×384+5×160 = **12087312 record bytes**. Final live set33+find/metadata slots gives **peak35**; output adds at most2 only after input release.
- Intern pool:26 fixed absolute paths+five row paths+five parents including empty root+safe-empty =37 distinct strings. Sum align8(32+2×UTF16 length) = **7080 path bytes**; inventory3586/scratch524288. These are Core accounting, not working set.
- Controller guards family5+case15 = **20**, case guards closed before Core. Baseline family+case16 directories/13 files/57 stream checks/3690 default bytes. Output adds one directory/file: **17/14/57/3690+N**. Named-stream0 describes checked inputs and controlled default-stream creation; no additional output ADS enumeration is claimed.
- Helper overhead outside Core arenas: fixed StringBuilder32768, ancestor list<=16, receipt/read scratch each<=65536 and bounded JSON DOM. No new Core payload buffer/input owner.
- AST0 errors:Core3 definition-only functions, worker51/supervisor28;13 shared allocation/oracle functions identical. All38 Core and30 helper keys covered. Embedded C# delimiters balance;11 unique usings/two fresh helper types/five fresh-type gates/three initializer calls. **No C# compilation result.**
- Public-data recount passed inventory raw pin, five payload lengths/hashes, tuple digest, single-case/three-output bindings, caps and whitespace. An initial independent tuple model omitted the actual ColdA-v1 NUL domain and failed; corrected source-derived model passed without source change or runtime.
- Frozen RootCopies25596B/`a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382`, deadline6038B/`0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45` and original output adapter reverified. Core inverse/HEAD/index/default diff/whitespace checks accompany callback; unrelated work is preserved.

No fixture/private path queries, .tmp creation/read, helper invocation, Add-Type/compile/native/CLI/Rust tests, product network runtime, keys/DB access, allocation, commit/push or ledger edits. Actual native error/Windows behavior, C# acceptance, dynamic N and duration remain unverified.

Later proposal only: **work20/whole30 seconds,lane1/retry0**, reports65536each/196608total; sources fit unchanged131072 caps. Reuse existing guardian/job/Pump/EOF/reap; no evidence yet warrants widening. C30 is **not reserved**; root handles activation/ceiling. Closed S5.3 C1572.2254909/F1562.2882567/P0; combined C2159.6741018/F6278.6084056/P0; ceilings1600/1642/0; active0/inactive C27.7745091/F79.7117433/P0. Whole S5.3/G1/S5.4 remain open.

Callback exact four pins/diff/ownership/profile, then **STOP for focused source review**, before runtime or publication.
