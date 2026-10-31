# C2-R / R1 source review

2026-10-31. Dormant source only under [accepted task](C-s53-cold-core-producer-size-01-attempt-01.contract-accepted.md). Added [fixture-support.ps1](C-s53-cold-core-producer-size-01-attempt-01.fixture-support.ps1): **56156B/SHA256 852310d69d030e8ce386c14aa35d06bcbfc1c585368e023beee230db8e4eda37**. Baseline0cbc322fe3ae06632af7ea4c1538f19b060d22ac; Core80889B/e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e unchanged. No Core/output/worker/supervisor edits.

## R2 interface and ownership

The PowerShell file defines only Initialize-S53ProducerSizeFixtureSupportR01. The function refuses cached helper types and contains the future Add-Type call; it was not invoked. No top-level compilation, filesystem access, environment probing or initialization.

Future synchronous C# lifecycle:

1. Construct Primitives with pinned accepted raw S53RootNativeR38.NewDirectory, DirectoryHandle, NewFile, Read(path,false), and exact protected three-SID Assert-FixtureAcl delegates. The helper invokes them only with its fixed synthetic literals. Constructor delegates are dependencies, not a public arbitrary-path file API.
2. FixtureSupport.Create(absoluteWorkDeadline,check,primitives) builds only the bounded oracle. No filesystem work until PrepareAndVerify().
3. PrepareAndVerify() creates fresh family/case, writes/flushes/readbacks every physical file, verifies the streamed inventory, then performs two complete independent bounded healthy traversals. Six fixture locks already exist and are closed afterward; no lock invention by Core.
4. ReleaseSetupGuards() closes all15 case guards, sets ReadyForCore only on known success; family/ancestor5 remain retained by this helper. R2 subsequently prepares/transfers E into Core before capture, using the contract's fixed mapping.
5. After Core comparison/receipts/disposal, VerifyReceipts(finalObservationUtf8,decisionHash,coreHash,factoryBefore,factoryAfter,attemptStartTimestamp) checks all four receipts independently. It creates no files, returns aggregate lengths/hashes in fixed owner/comparison/namespace/result order.
6. R2 must retain the helper in a try/finally and always Dispose(), including setup/verification failures. Dispose closes family/remaining guards and clears buffers. GetSummary() after disposal returns an independent aggregate snapshot; require Disposed, all known remaining counts0, no partial-return/create/cleanup flags, and the expected stage assertions. A ReceiptReport.Verified result alone does not certify whole-worker cleanup.

Every returned file/directory/current search/enumerator becomes reachable before validation or further callbacks. Successful closed file references are immediately cleared; no41745-object retained owner registry. Receipt owners occupy four preallocated slots. A failing SafeFileHandle getter cannot skip Stream.Dispose. FileStream disposal uses the accepted managed close/IsClosed evidence; it is not extra native CloseHandle instrumentation. Native directory and search close outcomes are checked explicitly once, never retried against potentially recycled values. Cleanup failures retain first type/HResult/native cause and prevent readiness/success; cleanup does not replace an already propagating operation exception. Unknown factory returns remain explicit, even if numerical known-return counters are zero. No deletion, retry, ACL modification of old parents, or repair.

Native held-info/default-stream code is the accepted C2-I primitive pattern, narrowed to fixed R1 scope: actual file type, no reparse/device, final path, volume/ID, link/length, create/write stamps, LastAccess excluded; bracket strict stream enumeration by held identity/stamp. Files require sole default stream and terminal38; directories require initial38;87/other errors fail. It does not call Core to establish premises or inspect production/private paths.

## Determinism, bounded metadata and ledgers

Oracle is independent of files/Core: explicit side/index order, four little-endian index bytes then zeros; two empty locks included in6956 rows. Constructor computes per-row digests and H and asserts the accepted H. Streaming writer uses exact compact producer schema, all three complete arrays, source/source-after derived zero provenance only at the two exact locks, actual provenance everywhere else. Every written byte is read back and checked against an independent expected pin; inventory parser uses a64KiB token window,4096-byte raw token cap/depth8/250000-token cap, strict UTF-8/Int64 types, duplicate-property masks and bounded seen map. No whole inventory string/DOM.

Expected pins remain3465145B/c40ffdedd028be593c08ded6b05daba44abe0d1a9e1dac86cb6d78632cbf3843 and H2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf. The contract/root's independent static calculations establish these expected values; R1 source has not been executed to claim a measured match.

Metadata:6956 rows/one ordinal dictionary,13915 file marks,6956 reusable array marks,13930 global IDs (13915 files+15 directories),15-slot traversal queue,20 setup/family guard slots plus one proof guard,6 lock stamps,11 namespace names,4 receipt slots. All insertions/entry counts are checked before retaining additional entries. The local owner-receipt six-ID set is bounded separately. Helper byte arrays total104894B (65536+6578+32768+12); an active inventory token reader adds65536B, so170430B explicit byte arrays at that phase. Writer flushes every inventory row; file-size charge precedes writes. One<=32768-byte receipt record DOM at a time, no receipt-wide text/DOM/line collection. CLR/PowerShell/Root FileStream/StringBuilder/JSON-internal and dictionary overhead is distinct; this is not a process-RAM guarantee or an alteration of Core arenas.

| Successful future count | Exact |
| --- | ---: |
| Created directories before E |16 (family1+case15) |
| Physical files / bytes |13915 / 94659943 |
| Create/write/readback owners |13915 |
| Independent premise-read owners |27830 |
| File owner returns/closes through setup |41745 |
| File owner returns/closes including receipt verification |41749 |
| Setup directory closes at handoff / family retained |15 / 5 |
| Total helper directory returns/closes after disposal |21 |
| Namespace enumerators through receipts |32 (30 premise+2 output) |
| Strict stream checks / successful search opens/closes |41784 / 41753 |
| Peak file / directory / enumerator / search owners |4 / 20 / 1 / 1 |
| Global identity entries / traversal queue |13930 / 15 |

Counters are incremented on actual ownership/results; GetSummary reports those counters and observed metadata/window/record peaks, not a synthetic success projection. The table is source-derived acceptance arithmetic, not a runtime observation. Work checks use the supplied common absolute deadline, including oracle/setup/hash/parse/verification; cleanup does not reset it.

Core production caps remain external/unchanged: rows8000,entries20000,handles40126,16MiB inventory,64MiB payload,96/32MiB paths/records,512KiB scratch,64MiB output. R1 validates final/private observations against the contract's exact Core13938 handle peak and other resource accounting; it does not implement Core ownership, sampler or deadline job. R2 must independently wire/check the scan_before13934-input/13935-with-E capture snapshot and full13912 notifications.

## Receipt checks and corrected fixed-record proof

Four receipt names only, all held concurrently read-only/deny-write-delete with E guard; exact ACL, native identity/default-stream checks, bounded before/after namespace scans, stamped read/hash/EOF and final reread/hash. Every6956 comparison row is checked in ordinal order against the oracle; both initial/final stamps, four hashes, provenance, six lock records, unique IDs and all11 namespace rows are checked. Fields/types/duplicates are strict. Final aggregate observation's38 fields are matched to current Core source; private result retains its intentional false/2/three-prior-digests state, final true/0/four digests/full byte totals. Private/final phase fields are13/14. Public reports contain only aggregates, never these per-entry stamps/paths.

**Narrow correction to the accepted contract's receipt upper bounds:** Core writes four FILETIME components as signed Int32, whose minimum uses11 characters. The earlier UInt32-width assumption was one character short per component. Static worst-value serialization gives:

| Receipt | Conservative emitted bytes, including LF |
| --- | ---: |
| owner |3891 (JSON3890) |
| comparison |11484356 =6956*1651 |
| namespace |7007 =11*637 |
| result, using fourteen128-byte numeric allowances |3983 (JSON3982) |
| Aggregate |11499237 |

The previous comparison1635/namespace629 bounds therefore must not be used by R2; no Core cap inflation is needed. Retaining the original owner/result generic32769 bounds instead gives11556901B, still below64MiB. These are upper bounds, not predicted native lengths.

Owner proof uses exact literal paths/attempt/schema and six zero-length/single-link stamps; each UInt32 uses10 digits, signed FILETIME11, file ID16 hex, start/deadline at most19 digits. Compact public static serialization totals3890 JSON bytes. Result proof covers all38 root fields, fixed success strings/five64-hex tuple digests, conservative19-digit nonnegative numeric fields, worst false booleans/nulls, four receipt hashes and fourteen phases:2190 non-phase bytes +14*128 =3982. Private result has fewer phase/digest fields, so is smaller. [System.Text.Json v10 JsonConstants](https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Text.Json/src/System/Text/Json/JsonConstants.cs) reserves128 for double formatting; [writer growth](https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Text.Json/src/System/Text/Json/Writer/Utf8JsonWriter.cs) uses256 initial/4096 growth hints. Fixed literal/property/string/number calls need no individual hint>4096. Both owner/result encoded bytes plus a4096 hint fit comfortably in Core's32768 record buffer. Actual measured receipt sizes/digests remain mandatory.

## Static validation and remaining R2 work

PowerShell ParseFile:0 errors, exactly one top-level FunctionDefinitionAst. Extracted C# text has balanced masked braces/brackets/parentheses; helper's38 observation field names/order exactly match current ObservationJson. No ReadAll/ReadToEnd, whole-directory array enumeration, delete, process/thread/task start, arbitrary File.Open, Core invocation or top-level call. All referenced files/pins/ledger checks were public reads/static arithmetic only. C# text checks are not compilation or Windows runtime evidence.

R2 still owns accepted primitive delegate bindings/source pins, closed Core/output mappings, allocation and -DateKind String timestamp boundaries, worker/supervisor integration, scan-before capture assertions,100ms OS observer and honest races/gaps, same-clock420/480 cleanup/guardian wiring, normal output-helper ownership transfer, actual native/tool returns and final root review. Frozen failing launcher-wrapper integration remains separately open before private. R1 does not certify compilation/native behavior, whole S5.3, private preservation or typed open.

Ledger unchanged: S5.3 C1634.6434909/F1562.2882567/P0; combined C2222.0921018/F6278.6084056/P0; ceilings1670/1642/0, active0. No allocation/runtime, fixture/events/.tmp/private reads, import/dot-source/Add-Type/compile, existing source edits, commit/push/navigation or S5.4. Callback then **STOP for focused R1 review**.

