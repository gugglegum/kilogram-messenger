# Cold-A attempt02: inactive preparation for root review

2026-10-31. Baseline **9df550e45839880f861680b24921d62fb950b402**. Production launcher/Core/output match that commit. [Timeout cleanup acceptance](C-s53-cold-timeout-controls-01-attempt-01.accepted.md) supplements the historical contracts/pins: three focused public controls passed, including late exit0 remaining failure and actual forced cleanup reap. This does not prove real private preservation or explain attempt01's45s overrun.

## Concrete inactive package

[D1 draft](P2-s53-exhausted-cold-a-01-attempt-02.D1.draft.json): **4770B / 4a3c76756b8a76628c7ea53d3903ca56e3b2097127bb62591510bb6fbaf6b5a0**.
[Native plan draft](P2-s53-exhausted-cold-a-01-attempt-02.native-plan.draft.json): **1365B / 6bee9fe846ce5421ed997e1ba2cd818a6d3ddaee05c4c798836fa0b09383c84e**.

Both actual JSON files have **activated=false**, outside the launcher's active D1 filename. D1 retains exact25 fields/32caps/18pin entries; native plan retains17 fields. All18 actual bytes/SHA pins are resolved at the committed launcher's fixed Pin-Spec paths.63 static comparisons confirm closed schemas,20 scalar bindings (activation intentionally false), caps, pin shapes/values, ordered write/report lists and native-plan linkage. No project function was executed.

| Key pin | Bytes | SHA256 |
| --- | ---: | --- |
| launcher | 76869 | 620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad |
| core | 82999 | eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f |
| output | 21721 | ced08d88656e7bad142d1e23a826c42e7be4f75da8380c838167610504db7ae0 |
| source_review | 16909 | 93178bb38eb5e031bb276261968feeb463b2be2dcb4fbf816f129ea762d719e4 |
| source_acceptance | 4655 | 85f7f1049712d8f2abeda152e4faa5c94e861813c85fdfca65f79807346329c0 |

The JSON contains all other13 actual pins. Historical fixed `P2-s53-exhausted-cold-a-01.integration-source-review.md` and `P2-s53-exhausted-cold-a-01.integration-source-accepted.md` remain unchanged; they describe earlier source revisions. Current timeout acceptance/new commit provide supplemental provenance, with no added D1 fields or rewritten history.

Serialization: UTF8/no BOM,2-space indent,LF,one final LF. The unique future `"activated": false` → `"activated": true` replacement was calculated **only in memory**: **4769B / 3dc9c86545b83cfbfc24ac14160a30f01a2c4a8da8044a57e763ad2ee9ae47fd**. Any edit/drift requires new hashes/review. No active D1 was read, created or activated here.

Prospective fixed active path: `E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\P2-s53-exhausted-cold-a-01-attempt-02.D1.json`. Future command, not executed:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1' -DecisionSha256 '3dc9c86545b83cfbfc24ac14160a30f01a2c4a8da8044a57e763ad2ee9ae47fd'
```

Cwd W below; default Supervisor/internal clock, **work45/whole60/lane1/retry0/no network**. After work expiry, failure stays sticky: Parent cleanup/reports remain bounded by50, independent Parent guardian51, Supervisor grace51/reap52/drain54/report57, independent child55/self59/outer60. Cooperative reports can now retain existing final Core phase/failure facts. Blocking I/O or slow cleanup may still prevent them; this run may fail/return unknown. No work-deadline increase or checkpoint telemetry.

## Exact same input scope; fresh outputs

W=`E:\AI-Projects\kilogram-p2p-messenger`. Private paths were not queried.

| Role | Fixed path | Permitted scope for a later allocated run |
| --- | --- | --- |
| O | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01` | Only two existing locks below and no-follow ancestry/identity; no O content traversal. |
| S | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb` | Complete bounded raw-byte test-state traversal/hash. |
| BKP | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01` | Complete bounded raw-byte preserved mirror traversal/hash, direct children. |
| Inventory | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\preservation-inventory.json` | Existing6765525B/SHA256 `c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3`; strict schema/three maps/provenance. |
| Output parent | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence` | Exact optional restricted create-if-missing; existing ACL unchanged. |
| Fresh E | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-a-01-attempt-02` | CreateNew protected evidence only; no attempt01 E reuse/inspection. |

Six **existing** read-only/share0 lock files; no creation/truncation:

```text
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01\one.state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01\two.state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb\one\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb\two\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\one\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\two\state\.kilogram-state.lock
```

Capture expected objects before four retained-handle passes:6956 files/45597399B per tree,182389596 hash bytes. Opaque test-key/credential envelopes are read as bytes **without decryption or typed DB/credential open**. Auxiliary local process observations are bounded4096; terminate only own attempt processes. No original O/S/BKP payload/ACL changes, copies/restore/delete/repair/recovery/rewrap or system changes.

Locks support cooperative-owner exclusion and observed entry sets, not continuous namespace absence or post-release state. Core releases inputs **before** receipt writes; the observation interval ends on release. LastAccess updates are excluded. Later typed open/ownership transition remains a separate gate.

Only four E receipts: owner.private.json<=65536B; comparison.private.jsonl<=33554432B; namespace.private.jsonl<=16777216B; result.private.json<=65536B, aggregate<=67108864B. Fixed public prefix `W\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\P2-s53-exhausted-cold-a-01-attempt-02` has exactly `.started.json,.claim.json,.p2.json,.parent.json,.supervisor.json`, each<=65536B/total<=327680B. Public aggregates/phase facts/hash bindings/bounded causes only, no private per-file metadata or input bytes. Existing closed Supervisor diagnostics map distinguishes bound observations from unknown; process exit is not proof of Core cleanup. Preserve partial evidence; no overwrite/reuse/retry of this identity.

## Scope and next gate

Standing [test-data authorization](S5.3-standing-test-run-authorization.md) covers these identified accounts/keys/copies; a new user-permission question solely for test-key bytes is unnecessary. **No F allocation exists in this preparation.** Inactive F23.9217433 alone is not an execution allocation; root will set internal budget after inspecting this package.

Unobserved: current input/lock/output existence/type/ACL/availability, inventory/payload equality and private I/O duration. Old attempt01 evidence and the accepted control artifacts stay immutable. Whole S5.3/G1–G4/typed-open/S5.4 remain open.

Only these three new inert documents are authored after exact23-file publication. No private stat/list/ACL/inventory read, process probe, new code, test/compilation/runtime, active decision/claim/E, nav/ledger or additional commit/push. Callback and **STOP before activation**.
