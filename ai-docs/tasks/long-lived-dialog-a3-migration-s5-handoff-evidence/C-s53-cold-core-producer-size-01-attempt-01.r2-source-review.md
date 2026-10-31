# C2-R / R2 source integration review

2026-10-31. Source-only delivery under [R2 task](C-s53-cold-core-producer-size-01-attempt-01.r1-accepted.md). Baseline HEAD0cbc322fe3ae06632af7ea4c1538f19b060d22ac. No allocation, compilation or runtime.

| R2 source | Bytes | SHA256 |
| --- | ---: | --- |
| [preflight](C-s53-cold-core-producer-size-01-attempt-01.preflight.ps1) |43579 |cd00433669109e304f5df0b35d55313119439f555cd6478d6274f77d958cae51 |
| [watchdog](C-s53-cold-core-producer-size-01-attempt-01.watchdog.ps1) |40557 |4dab51e6e413f0c8372be293bdbbc553532989bd51fb9690d78ff4e7c3b29148 |
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) |81411 |4aa345e8ec3a0a61b8b60b87d514d461fa76a2f78c56094c024b1114ce619515 |
| [output](../../../scripts/migration-s53-cold-output.ps1) |15663 |a53853c9dd25ae07ec4815787b0d3c592df4644ebd8e0c33c44cec7803d760ea |

Only these four sources and this new review changed in R2. Pinned [R1 helper](C-s53-cold-core-producer-size-01-attempt-01.fixture-support.ps1) remains56156B/852310d69d030e8ce386c14aa35d06bcbfc1c585368e023beee230db8e4eda37; [R1 review](C-s53-cold-core-producer-size-01-attempt-01.r1-source-review.md) unchanged. Root-native25596B/a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382, deadline6038B/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45 and fixed shell301368B/362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139 stay frozen.

## Minimal mappings and unchanged safeguards

Core adds only producer-size-r01 to its closed C#/PowerShell selector lists, enforces the exact3465145B inventory/c40ffdedd028be593c08ded6b05daba44abe0d1a9e1dac86cb6d78632cbf3843/6956rows/45597399B tuple, and maps it to the accepted fresh family/producer-size case and attempt string. Limits.Fixture falls through to unchanged production limits. No production/parser/capture/reader/scan/receipt/cleanup logic changes.

Output preserves its original public Proof literal, Create factory, wrappers and old behavior. A private activeProof is selected by a private boolean constructor; the new closed CreateProducerSize factory selects only the fixed producer-size proof literal. All existing instance checks use that selected literal. No arbitrary path argument or ACL weakening. Before-image15195B/81f625dd3334c5ce6ba5d7efb36d54404ccfefd1bbe03849312121a241da4dea was retained in the source comparison; this source was already untracked, so ordinary tracked git diff alone cannot describe its mapping change.

Static normalization removing only these additions and reversing Proof-to-activeProof substitutions reproduces both prior complete sources exactly. Core production caps/logic and output security/ownership/old wrappers are therefore unchanged. The frozen reflection wrapper's failing-entry ownership issue remains separately open before private; this worker does not call that wrapper.

## Worker lifecycle and exact acceptance

The entrypoints are future explicit -File scripts, not definition-only libraries. They have not been invoked. Core/output/R1 remain definitions-only until reviewed worker initialization. Supervisor arms the existing guardian pattern before public filesystem validation. Both processes validate a separately activated canonical root allocation and hold all eight pinned source files read-only; worker also holds the one-use started claim. No fixture access precedes claim/job/type/source gates.

Worker checks five fresh types, loads definitions, and initializes deadline once plus Core, Root-native, output and R1 once each. Definition AST shapes are3/3/1 functions for Core/output/R1. Deadline compilation is counted explicitly. Compiler diagnostics remain public-source-only, at most16 records/6144 total message characters, no runtime input messages. Delegate bindings are synchronous accepted Root NewDirectory(raw), DirectoryHandle, NewFile, Read(path,false), and exact protected three-SID ACL. No post-return callback is inserted between raw creation and R1's ownership registration.

R1 is owned by worker try/finally from Create through disposal. Setup returns two independently verified walks; all15 case guards close before Core factory while R1 retains5 family/ancestor guards. R2 compares the actual setup and final Summary fields, not a reconstructed success count. Final required values include41749 file returns/closes,21 directory returns/closes,32 disposed enumerators,41784 strict checks/41753 closed searches, all remaining0, partial/create/cleanup flags false and Disposed true. Metadata/window/record peaks are reported.

For setup timing, the pinned R1 interface exposes Stage through GetSummary. Only during PrepareAndVerify, Check reads that bounded aggregate, retaining four stage transitions (create,baseline,prepared,premises_verified), a probe count and total QPC probe duration. It derives separate observed creation/readback and two walk intervals. No stage snapshots are retained per file; probes stop before Core. These are first-Check observations after stage changes, including managed probe overhead, not new exact internal R1 timers or a speed claim.

Core is constructed with the actual factoryBefore/factoryAfter interval. Output owner creates restricted E, holds6 checked ancestors+E=7 handles, transfers E before capture, then retains6 ancestors. Worker keeps the same pending SafeFileHandle reference until BindOwnedOutput returns. On a bind exception it marks entry ownership unknown/failing, disposes Core first, then disposes only a still-open pending reference. This resolves this worker's local cleanup without certifying the old reflection wrapper. Any binding exception fails the attempt.

At scan_before hook, assert13912 file acquisition notifications, capture timestamp within the factory/deadline interval,13935 accounted owners including E (13934 inputs), zero payload passes/bytes. Snapshot resource peaks are handles13936,path563072,records18537456,inventory3465145,scratch524288. Hook sequence has13 phases and16 total hook/return boundary records; per-file acquisition increments only a counter.

Full-set-before-hash proof:26 fixed owners +13908 nonlock payload owners =13934 inputs. E adds1. Subsequent safe-empty S directory adds1, giving13936 retained inputs+E. A live tree search and current metadata owner add2: **Core exact accounted peak13938**, below40126. These are Core-controlled FS/search/receipt slots, separate from output/R1/public/runtime/OS handles. Safe-empty adds56 path bytes and384 record bytes, giving final563128/18537840. All fixed production limits remain unchanged.

Require normal Core completion, six existing locks, four passes182389596B, all five tuple digests H2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf, complete observed scans, final input release/no error/no unknown, final0 handles/four receipt digests. R1 receives actual final aggregate UTF8, allocation/Core hashes, factory bounds and common start, then independently streams all6956 comparison/11 namespace/six lock records and four receipts. Its provisional private result remains false/2/three prior digests; final is true/0/four/full totals.

Output ancestry identity/ACL is rechecked after Core, after receipts, and after independent verification;6 output-helper ancestors remain owned throughout that interval. Only then dispose output owner and require mask15/transfers4/remaining0/cleanup-known; R1 family guards do not substitute for these checks. Finally dispose R1, verify actual Summary, rehash all eight held sources, close public inputs, require job children0/Finish and release worker self object. Reported cleanup predicates are checked again by the supervisor with worker identity/UTC claim. Failed paths retain original sanitized cause and partial evidence, collect actual helper/Core cleanup observations, and emit bounded aggregate failure diagnostics. No deletion, repair, retry or catch-any PASS.

## Sampler and clock

Supervisor uses only the same retained started Process object, PID/native handle/start-UTC identity. It refreshes and queries OS handles/high-water working set on a nominal100ms schedule within its existing responsive pipe/deadline loop; no new monitoring process/thread/event. Drain/check before and after queries. At most4801 attempts; retain aggregates only.

A live API/identity/range error fails observer evidence. The exception race is narrowly accepted only for an innermost exact System.InvalidOperationException, no Win32Exception anywhere, followed by confirmed HasExited on the already fixed retained identity; ObjectDisposedException/Win32/access failures do not qualify. Report API, sanitized cause, confirmation and race QPC. If reads succeed but the post-query exit check says exited, discard that attempt as a live sample and report the distinct post_query_exit race. Pre-known exit and parent-observed exit stop all further metric queries. At least one valid live sample is mandatory; any other observer error prevents normal acceptance. Both policies remain untested runtime branches.

Report attempt/success counts, start/return QPCs, actual durations, maximum successful return-to-return gap and conservative measurement-gap upper interval, sampled peak handles and observed OS PeakWorkingSet64. Exit-tail includes last-return-to-observed-exit gap and an upper interval beginning at that sample's start; never certify the unobserved exit tail. OS memory high-water includes shell/compiler/setup/CLR/native overhead through observation; sampled handles may miss short peaks. It is not the exact Core peak or an arena/RAM promise. Existing retained Process is disposed only after reap/observations; stdio/public inputs/guardian ownership remain explicit.

Same QPC start is backdated to native supervisor process startup using the accepted Q bootstrap pattern, passed unchanged to worker, and used for Core/R1 deadlines, checks, waits and guardian ages. Report parent-before-spawn, worker-before-phase-clock and bootstrap/job/definitions/compiler/setup/factory/Core/receipt/disposal phases separately. The unchanged relative deadline helper receives420-Age and450-Age; main absolute checks/independent guardian prevent its construction latency from extending the attempt.

| Boundary, seconds from common start | Policy |
| --- | --- |
|420 |ordinary work cutoff; supervisor must observe normal worker exit/EOF before cutoff |
|450 |worker cleanup/report limit; parent bounded reap target |
|455 |independent guardian child cutoff |
|460 |parent pipe-drain target |
|470 |parent cleanup/report/check limit |
|475 |guardian self cutoff |
|480 |required future outer native tool limit, not activated here |

Work includes compilation, fixture/oracle/setup/two walks, Core, receipts and independent verification. Bootstrap reserve check is400. Normal return requires no force/unknown/cleanup kill, worker+supervisor exit0, empty worker pipes/EOF, actual helper/Core/output checks and source-after integrity. JSON observations alone cannot establish final success: report writes/guardian disarm can still fail, and root must inspect actual native/tool return. No clock reset, extension or per-phase allowance; blocked cleanup remains subject to outer protection.

## Future canonical allocation (not created)

Path: this prefix .root-allocation.json; command line AllocationSha256 is its actual raw SHA256. Strict duplicate/unknown/type validation, schema_version1; exactly24 fields:

| Fields | Required values/types |
| --- | --- |
|schema_version,activated |integer1 / JSON true |
|classification |root-approved-cold-core-producer-size-c2-r01 |
|root_thread_id |01a164ec-589c-7b41-bc34-3ba158d1f22b |
|attempt |C-s53-cold-core-producer-size-01-attempt-01 |
|operation_set |cold-producer-size-one-only-r01 |
|workspace |E:\AI-Projects\kilogram-p2p-messenger |
|fixture_parent |workspace\.tmp\C-s53-cold-core-producer-size-01-attempt-01 |
|fixture_cases |one-element string array producer-size-r01 |
|preflight_sha256,watchdog_sha256 |actual current source hashes above |
|core_sha256,output_sha256,root_native_sha256,deadline_sha256,fixture_support_sha256,shell_sha256 |fixed source pins above, lowercase64-hex strings |
|work_seconds,whole_seconds,retry_count,lane_count |integers420,480,0,1 |
|outputs |ordered string array started.json,producer-size.json,watchdog.json |
|public_report_bytes,public_total_bytes |integers65536,196608 |

Preflight/watchdog hashes come only from the pinned allocation; neither script embeds its own/the other's hash. There is no self-pin/circular dependency. Both validate fixed hashes for the six dependencies. Source opens are capped524288B, self/paired source opens131072B, reports/allocation65536B; actual source sizes fit. Eight source handles +allocation =9 public inputs, plus started claim in worker / worker report in supervisor =10 before close.

Public outputs are exactly the three new CreateNew files, each<=65536B, total<=196608B. Worker normal report conservative source bound is<=65200B: diagnostic38400 (6144*6 escaping +16*96 framing), two Core aggregates9000, two helper summaries4096, receipt800, two source-pin maps1536, fresh/init maps640, Core boundaries1536, fixture boundaries320/intervals512, phase map2560, two sanitized causes2048, remaining scalar keys/values plus clock fields<=3661. This bound intentionally combines outcomes that normally do not coincide. Fixed normal supervisor metadata is below16384B; unexpected pipe text is independently capped4096 characters/stream. All serialized report sizes are checked before write; over-cap/error is a refusal, not truncation or synthetic completion. Per-record receipts<=32768B; accepted corrected comparison1651/namespace637 bounds and conservative aggregate11556901 remain checked against independent measured sizes. No per-input name/ID/digest appears in public reports.

## Static checks and stop

ParseFile0 errors: worker28 functions,supervisor25,Core/output3 each,R1 one. All22 shared functions are byte-identical, including allocation/source/schema/counter checks. Core/output C# text has balanced masked delimiters; normalized prior-source equality verified. Encoding strict UTF-8/no BOM/LF/trailing-whitespace and source-size/pin checks pass. This is AST/text/arithmetic evidence, not C# compilation, delegate ABI, ACL/native Windows behavior, sampler branch exercise or runtime success. No closed-negative matrix was repeated.

Ledger untouched: S5.3 C1634.6434909/F1562.2882567/P0; combined C2222.0921018/F6278.6084056/P0; ceilings1670/1642/0; active0/inactiveC35.3565091/F79.7117433/P0.420/480 remains prospective; root must authorize any ceiling/reservation/fresh activation after source acceptance.

Only scoped source/review files written. No fixture/event/.tmp/private operation, dot-source/import/Add-Type/compile/project/native/Rust/network runtime, allocation/ledger/navigation edits, commit/push or S5.4. Private preservation, typed open/G1–G4/final package remain later. Callback then **STOP for R2 source review**.

