# C2-T — dormant production integration source review

2026-10-31. SOURCE ONLY on HEAD `694bdcb482df42b1401c3303a73c74b9d90253cb`. Implements [accepted delta](P2-s53-exhausted-cold-a-01.integration-accepted.md) plus the separately authorized [fixed reserve amendment](P2-s53-exhausted-cold-a-01.control-reserve-amendment.md). [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md) as amended by [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) remains the contract. Four-file submission only: three scripts below plus this note. No compilation/dot-source/helper/native/process/private-path operation, grant/allocation, ledger/navigation edit or commit/push.

## PID JSON representation correction

During control-source preparation, a pure in-memory check in the pinned shell established that ConvertFrom-Json decodes ordinary JSON PID integers as Int64. Root separately authorized the single Await-Start predicate correction: exact Int32 or Int64 only, >0 and <=Int32.MaxValue, different from self.Id. Strings/double/coercion remain forbidden; own child PID/UTC, clock/pins and claim are unchanged. The control fixture is not cast or rewritten. Source and inert-example pins below supersede the earlier accepted launcher pin only for this correction; no runtime/private grant follows.

## Exact source

| File | Bytes | SHA256 |
| --- | ---: | --- |
| scripts/invoke-migration-s53-cold-preservation.ps1 (new) | 71277 | `12661c746bfad302d5dabf0acdb5ea621005f5650120300a5e971d7507e6d976` |
| scripts/migration-s53-cold-output.ps1 | 21721 | `331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98` |
| scripts/migration-s53-cold-preservation.ps1 | 82999 | `59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af` |

Frozen RootCopies25596B/a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382, deadline6038B/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45 and legacy launcher30733B/75c5f4a210ffb84bdca7b87966c8f3cd0638f16e97f14f5e8a195e0f5d075112 are unchanged. No Rust/CLI/typed component added.

[Output factory](../../../scripts/migration-s53-cold-output.ps1):87–91 adds only a private byte-tag constructor and path-free CreateProduction(deadline), selecting exactly existing Core production E. Removing these five lines reconstructs the published output file; all synthetic bindings, native methods and C2-S wrapper stay unchanged.

[Core](../../../scripts/migration-s53-cold-preservation.ps1):47–48,82 adds CombinedHandles40126/ProductionExternalReserve64 and selects production cap40062. Algorithms, fixture factory/caps and bindings are unchanged. Inverting only these declarations/selection reconstructs published Core. Production record precharge becomes **12069232B** (64 fewer72B slots); fixture default remains12073840B. No external parameter or reflective mutation. An earlier explicit handle_cap refusal is allowed; no subset/pass/provenance change.

## Launcher, ownership and boundaries

[Launcher](../../../scripts/invoke-migration-s53-cold-preservation.ps1): Validate-Decision161; Validate-Report/closed Core serializer; Initialize-ProductionGuards; Await-Start; Invoke-Parent; Check-ChildReports; Invoke-Supervisor; final independent guardian. All roles/paths/attempt/decision filename fixed. No path/command/Part override. Parent alone explicitly initializes five fresh types under the deadline job. Auxiliary bounded4096-process conflict checks occur only after claim/job and publish counts, never process lists.

Supervisor creates the one-use started file before spawn, retains the actual child Process/PID/UTC/handle, then writes its body with that child identity and closes the writer. Parent waits only for that public writer's sharing32/IOException-HResult-2147024864 to finish, validates exact own PID+UTC/common start/pins, enters the job and atomically CreateNew-claims. Wrong internal Mode/clock/PID/UTC or an existing claim refuses before process/private operations. This closes the old mere-Mode=Parent gap without an IPC framework. All JSON has duplicate/type/field checks; 25 exact D1 fields,18 exact pin entries(each bytes+SHA),32 exact numeric caps. Future source-review/source-acceptance pins are supplied by the finalized D1 at fixed public filenames; code has no hash dependency on its own review.

Production E has five ancestors, prepared owner6→post-Take5→final0. ParentGuards holds four existing prefixes before querying its single parent leaf. Existing parent: no ACL mutation, retain identity and owner/group/DACL snapshot. Missing parent: only the separate decision flag AND write class permit frozen restricted native creation; ParentCreated is set immediately after its native creator returns, before open/new-ACL validation. No recursive creation, existing-E reuse, attempt fallback, delete or repair. Five temporary guards stay until real owner holds its five ancestors and rechecks succeed, then explicit CloseHandle results are observed once. A closed/invalid/uncertain guard refuses; no recycled-handle retry. Failed frozen opener validation is conservative unknown. RootCopies creation/open/ACL primitives stay frozen.

Fresh Core → actual C2-S Bind wrapper → CaptureAndCompare → receipts → disposal. Binding failure after transfer leaves cleanup with that wrapper; no caller retry. Original bounded primary cause and wrapper diagnostics survive secondary cleanup errors. Comparison failure does not write speculative receipts. Core closes inputs inside CaptureAndCompare; receipts describe retained observations and cannot extend the interval or authorize typed open. Every created/partial parent/E flag remains truthful. Public read/stdio/process wrappers report managed closure separately, with managed native-close-observed=false; no independent native-close result is inferred from SafeHandle.Dispose.

## Combined bound, clocks, reports

| External controlled FS ownership (both processes) | Bound |
| --- | ---: |
| Supervisor public streams including current acquire |24 |
| Parent public streams including current acquire |24 |
| ParentGuards fixed prefixes/parent including current |5 |
| Output owner ancestors plus E/current receipt transfer |6 |
| Total |59<=64 |

Each public opening reserves before acquire and refuses at24; records bounded40. ParentGuards reserves before frozen opener, never exceeds5; owner6 is source-bounded. Actual persistent E is counted in owner before Take, Core after; transient receipt handoff reserve is conservatively overcounted. Thus Core<=40062 + external<=59 = **40121<=40126**, including refusal/current-owner paths;64 is fixed, unused slots are not borrowed. Process/job/stdio/CLR-native internals are separate, not exact OS FS or working-set claims. Pure arithmetic boundary check:40061+1 passes,40062+1 refuses before growth;40062+64=40126. It is not execution of Accounting. Existing Accounting.HandleTake source is unchanged.

Single common start/frequency. Work45; Parent job whole50/report cutoff50/self-guardian51; Supervisor reap cleanup52, EOF drain54, report57, guardian child55/self59; external whole60. Guardian starts before public parsing/compilation. Normal completion needs successful retained WaitForExit1..20ms, exit0 then bothEOF before45 and full child/source/cleanup validation. Reap is never inferred from HasExited/EOF; pre-cleanup first reason cannot be promoted. Requested waits are not hard real-time promises. No deadline reset/extension.

Five CreateNew public reports <=65536B each/327680B total, fixed role schemas. Core aggregate fields closed; nested error maps carry only bounded type/HResult/native code/fixed marks and<=16 chain nodes. Compiler diagnostics carry type/error flag only. Private child pipe text stays in bounded memory: public Supervisor exposes **character counts/EOF only**, never raw stdout/stderr, stack text or private paths. Four restricted private receipts retain64KiB/32MiB/16MiB/64KiB and64MiB aggregate caps. No per-file private metadata/envelopes/inventory bytes in public output. OS sampling100ms/max601, sampled handles45000/lifetime WS1GiB; not a hard memory limit or exact OS peak.

45/60 is prospective/unallocated, grounded in C2-R comparison10.1149615s rather than120.8945368s dummy construction. Actual private duration remains unmeasured; no quota estimate. Raw preservation stays cooperative-owner/observed-entry-set only, whole attempt and continuous absence not_established.

## Inert decision and exact invocation

The following is **not a D1 file or activation**. activated=false and unresolved review/acceptance pin lengths0 independently prevent use. Conditional parent creation is shown as a distinct proposed write. After source/native-plan review and fresh explicit private authorization, root must fill those two pins, set activation, recompute D1/plan hashes and allocate; old grants are consumed.

```json
{
  "schema_version": 1,
  "classification": "root-approved-s53-exhausted-cold-a",
  "root_thread_id": "01a164ec-589c-7b41-bc34-3ba158d1f22b",
  "activated": false,
  "attempt": "P2-s53-exhausted-cold-a-01-attempt-01",
  "operation_set": "cold-a-raw-preservation-r43",
  "workspace": "E:\\AI-Projects\\kilogram-p2p-messenger",
  "origin": "E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\migration-s5-l1\\F-exhausted-origin-01",
  "state": "E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\migration-s5-storage\\l1-exhausted-observed-NCjBHb",
  "backup": "E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\migration-s5-preserved\\F-exhausted-preservation-01",
  "output_parent": "E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\migration-s5-preparation-evidence",
  "allow_output_parent_create": true,
  "output": "E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\migration-s5-preparation-evidence\\P2-s53-exhausted-cold-a-01-attempt-01",
  "inventory_bytes": 6765525,
  "inventory_sha256": "c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3",
  "expected_files": 6956,
  "expected_payload_bytes": 45597399,
  "allowed_writes": [
    "output-parent-create-if-missing",
    "restricted-new-e",
    "restricted-e-receipts",
    "public-aggregate-evidence"
  ],
  "caps": {
    "inventory_bytes": 16777216,
    "rows": 8000,
    "entries_per_tree": 20000,
    "files_per_tree": 8000,
    "depth": 32,
    "component_chars": 255,
    "relative_chars": 1024,
    "final_chars": 32760,
    "payload_file_bytes": 67108864,
    "payload_tree_bytes": 67108864,
    "combined_handles": 40126,
    "core_handles": 40062,
    "external_reserve": 64,
    "public_handles_per_process": 24,
    "path_arena": 100663296,
    "record_arena": 33554432,
    "scratch": 524288,
    "read_buffer": 65536,
    "receipt_total": 67108864,
    "owner_receipt": 65536,
    "comparison_receipt": 33554432,
    "namespace_receipt": 16777216,
    "result_receipt": 65536,
    "public_report": 65536,
    "public_total": 327680,
    "processes": 4096,
    "diagnostics_entries": 16,
    "diagnostics_chars": 6144,
    "stdio_chars": 4096,
    "sampled_os_handles": 45000,
    "sampled_peak_working_set": 1073741824,
    "maximum_samples": 601
  },
  "work_seconds": 45,
  "whole_seconds": 60,
  "retry_count": 0,
  "lane_count": 1,
  "pins": {
    "launcher": {
      "bytes": 71277,
      "sha256": "12661c746bfad302d5dabf0acdb5ea621005f5650120300a5e971d7507e6d976"
    },
    "core": {
      "bytes": 82999,
      "sha256": "59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af"
    },
    "output": {
      "bytes": 21721,
      "sha256": "331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98"
    },
    "root_native": {
      "bytes": 25596,
      "sha256": "a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382"
    },
    "deadline": {
      "bytes": 6038,
      "sha256": "0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45"
    },
    "shell": {
      "bytes": 301368,
      "sha256": "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139"
    },
    "r42": {
      "bytes": 20846,
      "sha256": "20e427de24a510c12fbd328350edb61bdc9d8c2e8752291423c37eaf6d9655c1"
    },
    "r43": {
      "bytes": 9037,
      "sha256": "03e4954c7627b9074717231ba616c3725dc7653b8382ad318c0da0c44d595bb3"
    },
    "r44": {
      "bytes": 7498,
      "sha256": "eccdf671997812dd699e99fc7e600e178f208b0cd2b5340863707b7fed03ff79"
    },
    "r41": {
      "bytes": 7566,
      "sha256": "193436d3bd2ac5223ef4af74beaf18b66d32b59e50a303a52b137ff52d2c5780"
    },
    "small": {
      "bytes": 4869,
      "sha256": "8693c60a808f2c054871fd3f5c05d7ca3e79e08ef3e95e842a4a16bcbf8863a2"
    },
    "c2r": {
      "bytes": 3255,
      "sha256": "892b4085172b1c4a2b2cc4a4e9a6c162176f57f6417362be904781a6d21d03b2"
    },
    "c2s": {
      "bytes": 3704,
      "sha256": "b8a9d0532ea3f90209b0dd5bcc2d67fa270eda771460d3ac7e1f3f7763eff5ce"
    },
    "delta": {
      "bytes": 10614,
      "sha256": "26bd1c419b61cc35eb11612cc38648c3c2b4d530c69e571468e51b938ac2641c"
    },
    "integration_accepted": {
      "bytes": 4365,
      "sha256": "b0611fef9a040cfe6dc1189f48c59a82dc910009789835ee7559f8cb8a4e58ab"
    },
    "reserve_amendment": {
      "bytes": 2567,
      "sha256": "0d4cb8e9f127454692b88d6977b45c82520274d8d1dbf804cea1639a76db842b"
    },
    "source_review": {
      "bytes": 0,
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    },
    "source_acceptance": {
      "bytes": 0,
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    }
  },
  "public_outputs": [
    "started.json",
    "claim.json",
    "p2.json",
    "parent.json",
    "supervisor.json"
  ]
}
```

This code-block decision would be4763 UTF8/LF bytes/SHA256 c5e1c9081ffd0c342c94eb0d22b6979f182cf8470f18006ebe4b846761d0163d. It is never written by this task. The corresponding equally inert native plan is:

```json
{
  "schema_version": 1,
  "classification": "source-only-cold-a-native-plan",
  "activated": false,
  "workspace": "E:\\AI-Projects\\kilogram-p2p-messenger",
  "shell": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
  "entry": "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s53-cold-preservation.ps1",
  "decision_path": "E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01-attempt-01.D1.json",
  "decision_sha256": "c5e1c9081ffd0c342c94eb0d22b6979f182cf8470f18006ebe4b846761d0163d",
  "arguments": [
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-Mta",
    "-File",
    "E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s53-cold-preservation.ps1",
    "-DecisionSha256",
    "c5e1c9081ffd0c342c94eb0d22b6979f182cf8470f18006ebe4b846761d0163d"
  ],
  "work_seconds": 45,
  "whole_seconds": 60,
  "lane_count": 1,
  "retry_count": 0
}
```

Concrete inert command (D1 file deliberately not created):
```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1' -DecisionSha256 'c5e1c9081ffd0c342c94eb0d22b6979f182cf8470f18006ebe4b846761d0163d'
```

## Checks and the one remaining evidence question

Three PowerShell AST parses0 errors; three embedded C# ParseText literals0 syntax diagnostics. No semantic compilation claim. Disk SHA256 matches independent pure-JS hashing. Core inverse check passes; fixture limits/algorithms unchanged. Output inverse and source reserve/geometry/schema calculations are static only. Frozen files unchanged; scoped diff whitespace and new-file UTF8/LF checks apply. No private path existence/ACL/duration has been observed.

New behavior needing focused evidence is the private-control launcher handoff/conditional parent guard, not the already closed comparison matrix. Minimum future C proposal: separately reviewed fixed public mappings to the **same** ParentGuards implementation, one missing-parent allowed-create case and one existing-parent ACL-preservation case; include wrong-PID/internal-Parent preclaim refusal in that bounded protocol check. Verify5guards/6owner geometry, created-vs-existing flags and native5close results. No inventory/input tree, Core hash benchmark or private path. A later authorized in-memory compiled check of the production40062 edge can complement the source arithmetic without filesystem work. Such mappings/test wrapper, compilation and runtime are **not authorized or added here**; root decides the smallest focused source/activation after this review.

Typed credential/DB open, decrypt, recovery/repair/rewrap and the post-release ownership gap remain later explicit gates. No G1–G4/private grant/S5.4. Callback root and STOP.
