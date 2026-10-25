# A3-S5.1 — storage migration implementation

2026-10-26. Implementation and bounded verification are complete; submitted for
root implementation review. This is not acceptance/publication. No commit/push
has been made for this candidate.

Baseline: `3547cf528f15c15a21cdb9e30b1d7ca35a01297a`.
Assignment: [storage task](long-lived-dialog-a3-migration-s5-storage-task.md).
Normative design: [accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md),
SHA256 `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`.

## Scope and owner boundary

This is cut1/storage administration. It implements production source validation,
Fence/Copy/Verify/Ready and offline capacity/history operations. It does not provide
the S5.2 live indexed L1 owner, finish an incomplete initial handshake, perform the
existing Consent2 P/A/K/Committed handoff or enable mixed live ingress. Storage
Ready is deliberately insufficient to launch a peer runtime. Full511, whole P5,
S2 100k and live receipt/C2 escrow-release gates have not been run or claimed.

The four root-owned dirty task/README/next-stage/coordination files are preserved,
not edited by this executor. They will be listed separately in the candidate
inventory. Accepted contracts/reviews/manifests, historical evidence and
development-environment.md remain protected.

## Implemented paths

| Layer | Change |
|---|---|
| [source](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/source.rs) | Existing key/DB only; bounded authenticated primary index, per-row AEAD/hash/typed cap, exact table cardinality and source identity. Saved original-mode ciphertext is the sole override on resume. |
| [private provenance owner](../../apps/kilogram-cli/src/ordered_dialog/storage_migration/provenance.rs) | Actual Device secrets/certificates/Root signatures/membership/Consent1, selected controls/initial transcript, public Native restore, curve/SessionKeys/OTK/SID, signed Event1 headers, receipts and actual pending O/A. No caller provenance boolean. |
| [readable fold](../../apps/kilogram-cli/src/ordered_dialog/storage_migration/fold.rs) | Two eligible frontiers, filtered continuation/own tie, causal and Guard2 preflight/apply checks, exact final Guard1 summary and seen generation/first. No packet replay. |
| [Fence](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/fence.rs) | One Immediate commit in the original DB: schema5 sentinel, closed Root7, Intent, source witness/budgets and replacement mode. Original claim/keys survive; storage incarnation is generated once. |
| [row recipe](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/copy.rs), [preflight](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/preview.rs) | Same bounded produced-row recipe for quota preview and actual copying; preflight caches only sizes/hashes. Fence requires its private matching preflight result and refuses insufficient quotas first. |
| [closed operations](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/operations.rs) | Bounded source batches/checkpoints, output digest, pause/blocked/resume, completeness and one-time Ready. Lifecycle/purpose checks precede duplicate shortcuts. |
| [records/budgets](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs) | L7R canonical local records; Migration23/global51 and LegacyBudget24/23+24c are explicit. Required owner/payload/summary/reservation/funding/Native/meta-cap correspondences are checked on load and commit. |
| [administration](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/admin.rs), [CLI owner](../../apps/kilogram-cli/src/ordered_dialog/storage_migration.rs) | Offline check/resume/status/pause/old-history/capacity CAS and exact profile publication. Profile3 explicitly cannot start a live indexed runtime. |

Source index is capped at 8MiB, total legacy plaintext at 384MiB, and individual
records at their V4 typed limits. Source/derived bodies are not accumulated as a
snapshot. DB caches are 64MiB; the existing bounded affected-plan checks remain.
Source provenance is verified under the same lifetime state lock which moves into
the repository at Fence. The original source remains a witness; Root7 is never
opened as a mutable old Engine or adopted as fresh Runtime6.

Component5/Runtime6 encodings and Native2 configuration remain separate. Shared
changes are the closed Root7 dispatch, explicit global Author/Migration handling,
a bounded scratch-tree fork for the digest, streaming/capped V4 journal recovery,
and repaired writable, synced atomic pause markers. Common append/page/allocator
algorithms were not changed; no100k run is justified by this diff.

## Provenance and accounting

LegacySnapshot is emitted only by the closed source-copy lowering. Mandatory O and
actual required R must exist and authenticate. Own A is NotApplicable; uncaptured
peer A/F are UnrecordedLegacy. An auth-catalog entry is not a fabricated capture.
Own unacked R remains NotApplicable and its debt is retained separately. Pending
retains its original event, actual O/A, head/ordinary classification, charge and
quarantine checkpoint. All readable snapshots and normalized projections are
compared to the immutable source and actual self-HPKE plaintext before Ready.

Per occupied C, B4 is147980288 bytes. Covered U/R reduces Held escrow; the budget
leaf pays its4096-byte metadata capacity plus585 charge overhead, remaining hold
and pre-Ready64MiB. Ready moves that endowment to free exactly once in the same
commit as current-pointer publication. Imported head occupancy is recorded during
Copy, so unused-head temporary space is not paid a second time. Empty storage
slots have no LegacyBudget or endowment. Global source backing belongs to owner0
and does not pay another C's hold.

Retained source key/value lengths, old index/generation bindings and the replaced
mode entry are counted once in SourceBacking. Original manifest/mode bytes saved
only in Intent are not charged again as backing. Actual DB length, retained
filesystem shadows, managed public output allowance and256MiB headroom are
separate physical liabilities. The preflight growth forecast is not an OS space
reservation; the backend enforces actual requested growth and observed free space.

Capacity changes require expected root generation, config revision/hash, matching
identity/paths and componentwise nondecreasing capacities. A committed exact retry
is idempotent. Changed stale requests, decreases and path retargeting fail. No
extra repair endowment or migration incarnation is created. Reopen resolves IO
ambiguity before another write; a poisoned handle cannot return success or write
a blocked checkpoint over an unknown outcome.

Public profile publication uses the latest committed plan. Its fixed reserved
`<output-profile>.kilogram-migration-stage` path prevents an unbounded collection of
orphan temp files across kills. Write/sync/atomic publication is retained. An
already-published different file requires the explicit expected raw BLAKE3 hash;
ordinary resume returns ReadyNeedsProfileUpdate rather than silently replacing it.

## Operator surface

All storage commands require the existing state directory, expected Account and
Device. Configuration is bounded JSON with unknown fields refused:

```json
{
  "logical_quota": [17179869184, 17179869184],
  "physical_capacity": 51539607552,
  "output_profile": "E:\\public\\indexed.profile",
  "control_ipc_file": "E:\\public\\indexed.ipc",
  "locator_directory": "E:\\public\\indexed.locators"
}
```

Public path parents must exist; paths must be absolute, normalized and outside
state. Existing profile/IPC targets must be regular files<=64KiB and a locator
target must be a directory. Invalid types, inside-state outputs and staging-path
collisions are refused before Fence. The fixed staging name must be unoccupied
before Fence.

- `ordered storage-upgrade ... --config-file ... --check`: validate/preview; no Fence.
- `ordered storage-upgrade ... --config-file ... --resume`: exact source resume and
  latest-plan publication. A different ordinary config requires explicit capacity CAS.
- `ordered storage-upgrade-status ...`: authenticated stored checkpoint and per-C
  native phase, debts, hold and funding; runtime_available=false.
- `ordered storage-upgrade-pause ...`: persist pause on a stopped, exclusively
  locked repository; an already-running owner returns Busy.
- `ordered storage-history ... --context ... --offset ... --limit ...`: immutable
  V4 offset dataset only, up to1024 metadata entries/C and8 output items/48KiB
  escaped JSON. No fabricated cursor or SID. After the one authenticated inventory
  load, its decoder requests only the selected C's bodies; incomplete SID returns
  HistoryUnavailable/NeedHandshake.
- `ordered storage-increase-capacity ... --config-file ...
  --expected-root-generation ... --expected-config-revision ...
  --expected-config-hash ...`: explicit capacity/recovery purpose.

This slice exposes the offline command path. It does not advertise an unfinished
MigrationAdmin IPC3 server. The control IPC path is a pinned future publication
path; the existing Runtime6 IPC2 protocol is unchanged.

## Evidence and reproducibility

All contributing attempts, including failures and setup, remain in
[the ledger](long-lived-dialog-a3-migration-s5-storage-evidence/contributing-cases.json).
Case JSON records full runner elapsed, sources and fixed image hashes. The stable
wrapper's compile/stage/copy/recovery/cleanup overhead is charged inside a run;
separate compile-only/static steps are identified separately.

Frozen d9b image remains61531648 bytes,
SHA256 `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
The field CLI and control peer use distinct fixed paths under
`target/compatibility/long-lived-dialog-migration/`. They are the same current
public CLI image: the second process is a real ALPN1 peer whose repaired debug
schedule withholds actual controls. No pause flag is passed to frozen d9b. Parent
evidence proves child liveness at the marker and actual kill/wait. No private
Native/Head/Event rows are manufactured for positive origins.

The real stopped small seed is new `.tmp/migration-s5-storage/feasibility-01`;
source branches run one Native domain at a time and are never merged. New partial,
kill and renewed-authority fixtures and their parent-result.json files stay under
that same new fixture root. Accepted historical DBs are not opened.

## Verification result

Final image:67693056 bytes, SHA256
`62b2bb9a7f4e2793b30274593fe501e2863c884b07d204d4731753c31a8fafba`.
[Field image record](long-lived-dialog-a3-migration-s5-storage-evidence/static-field-images-final-04.json)
also records the unchanged d9b/S3/exact S4/relay images. Archived S3 source ZIP
still hashes to `f43ebc77e8d3a170661f30ceb90e5eca9affc6f106a67c5b6754aa3b27d54abb`;
the frozen helper is the accepted7f82036 build, not a renamed current binary.

| Final case | Runner seconds | Result and actual assertions |
|---|---:|---|
| F-storage-final-03 |54.979485|5/5 owner/core tests: source keys/Native/fold, exact need/need−1, clean reopen, actual primary corruption vs stale shadow, required O/R negatives, copy closed before mapper, profile/CAS, IO poison/reopen, dependency/seen/genesis fold negatives and C1-only decoder sentinel. |
| F-process-final-04 |174.911958|5/5 process tests: five actual d9b partial boundaries, six parent kill boundaries, staged/committed capacity outcomes, exact retry/stale refusal, genuine renewed O/A with ordinary pending+own unacked debt, real two-C escrow/history and actual d9b/S3/S4 version/Busy fences with positive controls. |
| F-two-context-final-02 |47.125874|Final fixed image: actual two C, independent C1 need−1 refusal, exact need, two funded pools, exact scoped history and explicit live-runtime refusal. |
| C-storage-invariants-01 |22.934646|2/2: canonical codec/caps, budget payload/owner/summary/missing leaf, no cross-C backing payment, scratch digest parent membership, no second funding. |
| C-runtime6-regression-01 |42.000594|8/8 fresh Runtime6 layout/metadata/purpose-before-retry/policy/lease regressions. |
| C-component-regression-01 |93.437588|16/16 Component5 root/credit/integrity/IO/capacity/child-kill regressions. Includes4015-row single-growth fixture, not100k. |
| C-ordered-recovery-01 |15.723898|3/3 V4 recovery/fence/reservation tests; maximal index is metadata-only18958 records. |
| C-native-regression-01 |19.532496|1/1 real Native account/session restore and duplex across eight DH changes. |

The exact final source snapshots are F-storage-final-03,
F-two-context-final-02 and F-process-final-04. Their current-file comparison is
empty: [source correspondence](long-lived-dialog-a3-migration-s5-storage-evidence/final-source-correspondence.json).
Earlier differing snapshots are explicitly historical and are not substituted for
the final code's checks. Shared regression snapshots cover the respective touched
branches. Every final process case records source and unchanged fixed EXE hashes.

In final process evidence, the parent observed the child alive at each synced
checkpoint and killed/waited for that owned handle. Fence-staged remained V4;
Fence-committed/mid-Copy/Ready-staged resumed whole Root7 checkpoints;
Ready-before-profile remained storage Ready without a public launch profile.
Middle Copy was actually partial, and pause/reopen/resume retained source and
incarnation. Ready retry did not recopy or fund again. All six exact old history
comparisons retained IDs, requests, UTF8 and deterministic order. Small-case
PrivateMemorySize64 pause samples were9564160–10534912 bytes; these samples are
not a whole-scale or worst-case memory claim.

Final C total is193.629223/600 seconds; F is811.432707/1200 seconds, including
all earlier contributing attempts and failed fixtures. P is0/2400, not run.
Compilation/static-only work is separate. Preserved failures:

- F-partial-origins-01: fixture paths contained `apps/../..`; the normal config
  path guard refused them before Fence. The marker/liveness/kill infrastructure
  itself worked. The controller now supplies normalized paths.
- F-renewed-obligations-01: real reconnect delivered the first own receipt, so the
  fixture had pending but no remaining own debt. A later real Text was added to
  capture simultaneous pending/unacked; no source row was fabricated.
- F-core-smoke-01: the Rust test passed; PowerShell postprocessing subsequently
  failed on an empty C aggregation. Original JSON/log and the separate runner
  error remain. Aggregation was fixed without silently rerunning that case.
- Intermediate compile/Clippy diagnostics remain beside the final passing logs.

Strict Clippy passed affected state/ratchet/CLI packages with all targets and exact
`-- -D warnings` forwarding; see static-clippy-final-06.log. Locked/offline
verification builds and stable compile-only staging passed; fmt and diff checks
passed. All network harnesses ran only through the stable wrapper, Jobs12,
BelowNormal. No full511, whole P5,100k, cloud/network dependency fetch, commit or
push was performed.

## Reproduce and review

Use new run directories. First create the small seed with the unchanged frozen
d9b image and `scripts/test-ordered-dialog-process.ps1 -Scenario Duplex
-MessagesPerLane 2 -Burst 1`; charge that whole setup as the feasibility case.
Then build/stage with `scripts/stage-migration-s5-images.ps1` (unique evidence name)
and compile the three affected packages with `scripts/run-cargo-tests-stable.ps1
-CargoJobs 12` without Run. Fixed-path process inventory must be empty before
recopy. Run selected cases through `scripts/invoke-migration-s5-case.ps1`, passing
the new seed as FixtureDirectory; use new case IDs and retain failed logs. Select
`ordered_dialog::storage_migration::tests::` for the five owner/core cases and
`migration_s5_process_` for the five process cases. The latter creates its partial,
renewal and two-C positive origins through real CLI/ALPN1 actors.

[Final preservation/checks](long-lived-dialog-a3-migration-s5-storage-evidence/validation-final.json),
[protected file hashes](long-lived-dialog-a3-migration-s5-storage-evidence/protected-files-final.json)
and [candidate inventory](long-lived-dialog-a3-migration-s5-storage-candidate-files.json)
provide review boundaries. The inventory contains raw SHA256 and normalized Git
blobs, labels the four preserved root-owned paths, and excludes its own checksum
cycle. HEAD remains the baseline; index is unstaged. Publication requires root
acceptance and its exact allowlist.
