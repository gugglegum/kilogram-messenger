# A3-S5.1 storage implementation — review 1

2026-10-26. Root decision: **changes requested; implementation not accepted**.
The accepted S5 architecture remains unchanged. This review authorizes the bounded
repairs below in the existing GPT-6.1 Sol Max executor chat, normal speed, no Fast.
No S5.2 or implementation commit/push is authorized by this review.

## Reviewed submission

- Baseline/HEAD: `3547cf528f15c15a21cdb9e30b1d7ca35a01297a`.
- [Storage assignment](long-lived-dialog-a3-migration-s5-storage-task.md).
- [Accepted contract](long-lived-dialog-a3-migration-s5-contract.md), SHA256
  `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`.
- [Implementation submission 1](long-lived-dialog-a3-migration-s5-storage-implementation.md),
  SHA256 `4288cd20d4fb89f967203333e26436f647fb970d495d4b0b8a147c2f98eaa85b`.
- [Candidate manifest 1](long-lived-dialog-a3-migration-s5-storage-candidate-files.json),
  SHA256 `3b5c1b85601e9d59415195d3975721ee086e3ff93fbe679ec7061dd7fcb950a1`:
  147 listed files plus the manifest itself, 148 paths total.

Root inspected the source, process-kill assertions, evidence and budget ledger.
Two read-only GPT-6.1 Sol Max reviewers independently covered provenance/fold and
state/ledger/lifecycle. Root did not build, run product tests, edit product code or
open any database during this review. Negative scenarios below are findings from
code inspection, not claims that root executed a reproducer.

No additional actionable defect was found in the inspected Fence/Copy/Ready
atomicity, purpose-before-retry checks, source backing ownership, per-C escrow,
one-time Ready funding, capacity CAS or publication recovery. That observation is
limited to this storage slice and is not full S5 or live-runtime acceptance.

## Recovery check after the reported power interruption

Before resuming work, root independently verified:

- All 147 listed sizes, raw SHA256 values and normalized Git blobs match submission
  1; the actual changed-path set is exactly the expected 148 paths.
- The manifest, report, accepted contract and protected development-environment
  file match their pre-interruption hashes. The latter remains
  `53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f`.
- All 62 JSON evidence files parse. All six fixed current/frozen executable sizes
  and hashes match the final field-image record.
- `git fsck --full --no-reflogs` exits 0. HEAD is unchanged, the Git index is empty,
  no `.git/*.lock` file remains, and `git diff --check` passes.
- A process inventory found no cargo/rustc/kilogram process; the executor's last
  turn was completed and it is idle at review.

There is no detected partial write in the reviewed source/evidence/image package.
This is not a filesystem-wide or disk-health assessment. Ignored fixture database
contents were not opened or certified by these checks. Before reusing a stopped
seed, validate it through the supported path on an isolated new branch, with its
existing provenance and exact logical expectations; do not silently repair or
overwrite the original. Charge any new setup/recovery attempt to its proper set.
Historical accepted S1/S2/P/F databases remain excluded.

## R1 — P2: recovery reads the old index before enforcing its cap

At [ordered.rs](../../crates/kilogram-state/src/vault/ordered.rs),
`recover_ordered_journal` lines 717–730 caps manifest/generation, then calls
`is_ordered_profile`. That method at lines 63–65 calls `load_manifest_index`, which
copies/decrypts the complete metadata value. The `INDEX_CAP + 41` ciphertext guard
is reached only later in `restore_shadow`, lines 744–755.

A stopped V4 state with a genuine prepared version-2 journal and an oversized
manifest-index ciphertext therefore reaches allocation/decryption before bounded
refusal. `StateDirectoryLock` runs this recovery before the new bounded migration
source verifier. This violates the accepted contract section 2 recovery bound.
The demonstrated reasoning concerns corrupted local persistence; this review does
not claim a remote attack.

Required repair: enforce the index ciphertext bound before any profile helper can
load/decrypt it, preferably using one shared bounded authenticated index load rather
than parsing twice. Retain schema dispatch and primary-authoritative recovery.

Required regression: a new disposable V4 fixture with a real prepared journal and
an index ciphertext of `INDEX_CAP + 42` bytes with the supported envelope prefix
must return the cap error before decryption or shadow mutation. Assert unchanged
primary/shadows and include a valid recovery control. A focused local state test
is sufficient; this does not need another network fixture.

## R2 — P2: archived locators and peer authority skip semantic verification

At [provenance.rs](../../apps/kilogram-cli/src/ordered_dialog/storage_migration/provenance.rs),
`VerifiedSource::verify_scope` verifies enrollment, context specs, selected controls,
events and auth-catalog records, but has no checks for source kind 23 locators or
kind 31 peer-authority snapshots. The structural classifier in
[source.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/source.rs)
and opaque archival branch at lines 662–672 of
[copy.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/copy.rs)
do not supply those signature/account/context checks.

Two concrete substitutions can pass that semantic gap when the primary AEAD/index
is coherent: a genuinely signed C2 descriptor stored at C1's peer-locator path,
and a genuinely Root-signed foreign-account snapshot stored at an expected peer's
authority path. Ready must not certify these as a correctly scoped source.
Accepted contract section 3 explicitly requires locator signatures/roles/membership
and authority/account binding, even though these are historical witnesses.

Required repair: statically decode and verify every saved local/peer locator using
its context and expected role (`Descriptor::parse` already supplies those checks).
Verify peer-authority Root signatures and account-to-path/registered-peer binding.
Check the other known source kinds for equivalent opaque semantic gaps and document
the per-kind coverage; report a design contradiction before expanding semantics.

Required regressions: coherent authenticated source substitutions above must fail
before Fence, and the same static verifier must protect resumed verification before
Ready. Add positive controls showing that a correctly signed expired locator and
a correctly scoped authority snapshot recording a revocation remain archivable.
Do not call `valid_now` or require current messaging permission for these historical
witnesses. Do not manufacture positive Native/Head/Event origins, invoke packet
crypto during migration, or turn archived authority into current live permission.

## R3 — refresh final-source verification and field-build binding

The reported `C-storage-invariants-01` pass predates changes in
`migration7/records.rs` and `migration7/preview.rs`, as well as four owner/test files.
Root independently compared that case's source manifest with the submitted files.
The final-source correspondence labels the older snapshot historical, but no
branch-level reuse argument establishes the final ledger invariants.

After R1/R2, rerun these two short storage-invariant tests on the final candidate,
plus the targeted new negatives and affected ordered recovery controls. Record
actual assertions and source hashes. This is a verification requirement, not a
claim that the earlier tests failed.

For the repair's field build/staging, record baseline, exact build arguments,
source hashes before and after the build/stage, and the copied fixed-image hashes.
Existing case snapshots establish which sources were present during execution;
this added binding makes the final executable's build provenance explicit. No
stale-field-binary defect is asserted merely because the normal Cargo binary and
the fixed field image have different hashes after integration-test compilation.

Run final owner/core and real-process groups on that final candidate. The separate
two-context field repetition is unnecessary when the final process group includes
that same scenario. Unaffected shared regression evidence may be reused with an
explicit source/branch comparison. Run the affected locked/offline build, strict
Clippy with exact `-- -D warnings`, formatting and diff checks. On Windows use
compile-only Cargo and the stable wrapper; verification profile, Jobs12,
BelowNormal. Never recopy a live fixed-path executable. Independent fixtures may
run concurrently only when image staging and Native-domain ownership are disjoint.

## Budget and resubmission

The existing contributor ledger recomputes to C = 193.6292229/600 seconds,
F = 811.4327066/1200 seconds, P = 0/2400 seconds. Remaining F is 388.5672934 seconds.
Prior owner/core + process groups took approximately 230 seconds combined. Use a
bounded repair plan and retain all setup, failed attempts and complete runner time.
Do not repeat the separate two-context case, full511, 100k or P5. If the forecast
does not fit, report to root before a long run; do not reset or silently exceed the
ceiling. Later whole-S5 feasibility will need a separate root budget decision.

Preserve submission-1 report/manifest and every prior per-case/static evidence
file. Before updating a cumulative ledger, save its submission-1 snapshot under a
new name; cumulative updates must retain every old contributor. Use fresh case
IDs and fresh final-source/build evidence names. Submit a new
`long-lived-dialog-a3-migration-s5-storage-implementation-revision-2.md` and
`long-lived-dialog-a3-migration-s5-storage-candidate-files-2.json` with exact sizes,
raw hashes and normalized Git blobs, including this root-owned review.

Preserve the four root-owned assignment/navigation files and this review byte for
byte. Preserve accepted contracts/manifests, protected historical evidence and
unrelated work. Send one substantive callback to root chat
`01a164ec-589c-7b41-bc34-3ba158d1f22b` with R1/R2/R3 resolution, final tests, cumulative
budgets, manifest hash and remaining limitations. Stop for implementation review.
Only root acceptance followed by an explicit publication assignment authorizes
commit/push of the exact accepted diff.
