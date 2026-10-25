# A3-S5.1 storage implementation — revision 2

2026-10-26. Candidate for implementation review; not accepted or published.
Baseline/HEAD remains `3547cf528f15c15a21cdb9e30b1d7ca35a01297a`.
This revision addresses [review 1](long-lived-dialog-a3-migration-s5-storage-implementation-review-1.md)
under the unchanged [accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md).
S5.2, live indexed runtime, commit and push remain outside this assignment.

## R1: bounded ordered recovery

`EncryptedStateVault::bounded_ordered_profile` is now the shared metadata path for
profile dispatch, v2 journal recovery and shadow restoration. It checks borrowed
redb manifest/generation/index lengths before copying or decrypting their bodies.
The index ciphertext limit is `8 MiB + 41`; recovery reuses the authenticated
decoded index rather than invoking an earlier unbounded profile helper.
Manifest MAC/schema dispatch and primary-authoritative, two-pass shadow recovery
remain in place. `decode_generation` is an extraction of the previous generation
checks, shared with the bounded path; its authentication rules did not change.

The new local regression creates a disposable V4 primary with a genuine prepared
version-2 journal. It substitutes an index ciphertext of exactly **8,388,650 bytes**
(`INDEX_CAP + 42`), retaining the supported envelope prefix. Both the profile
helper and actual `StateDirectoryLock::acquire` return the exact
`index recovery cap` error. All primary metadata/record bytes, stale shadows and
the prepared journal remain unchanged. Restoring only that disposable fixture's
original index lets the same genuine journal recover the primary shadow, remove
the stale queue shadow and retain the original generation and primary digest.
This is a persistence corruption test, not a claimed cryptographic Native origin.

## R2: static historical witness verification

Every saved local/peer locator now passes `Descriptor::parse(spec, expected_role,
raw)`, including Device signature, context, certificate/role pins, encoding and
address/revision/interval shape. The production verifier does not call `valid_now`.
Every peer-authority snapshot passes Root verification and exact encoding, and
its account must match both its path and a registered context's opposite account.
Historical snapshots are not checked for current messaging permission.

The same verifier is used before Fence, on resumed retained source before copy
and Ready, and for the selected context in offline history. A scoped history read
only fetches that context's locators and its peer's authority; the existing C2-body
sentinel remains covered.

Two new coherent authenticated-primary substitutions use `OrderedVault::commit`,
which recomputes the source AEAD/index/generation:

- A genuine C2 public Spec/Root consent ceremony and actual peer Device signature
  produce a descriptor that is valid for C2, then it is stored at C1's peer path.
- The genuine own Root-signed snapshot is stored at the opposite account's
  peer-authority path.

Both fail through the real owner before Fence, with unchanged V4 source ID and
generation. Each is also fenced/paused through the structural state layer to
model a checkpoint produced by the previous incomplete owner. The Native, Head,
Events and fold facts remain from the real d9b origin; only the source ID is
updated to identify the deliberately substituted archive. Resuming through the
real owner persists `Blocked` with the exact static reason, no Ready endowment
and no public output profile. No positive Native/Head/Event is manufactured.

The positive control installs a correctly signed expired local locator and a
correctly scoped peer snapshot that actually revokes the peer Device. The test
separately proves expiry/current messaging denial, then the static verifier,
scoped history and complete owner migration succeed through Ready. Both signed
witnesses remain byte-for-byte readable from retained source. Migration Native
constructor/packet-crypto counters remain unchanged in negative and positive cases.

### Audit of all known source kinds

The audit also closed equivalent opaque paths: unexplained archived controls are
rejected, saved `current-auth` uses the existing static Hello/Proof transcript
decoder, fork witnesses have bounded known-reason shape and signed proof/scope
checks, global Auth catalog entries require a registered certificate, and the two
legacy ratchet sentinel records require their exact original constants. These
checks interpret existing encodings; no wire/storage format or current live
permission rule was added. Fork evidence may retain the old cross-C Event in an
author-sequence conflict, verified by the same pinned Device; it does not become
a readable Native/Event origin or a live lease.

| Kind | Source verification and archival interpretation |
|---|---|
| 0 | V4 mode exact; retained original mode override only on authenticated resume. |
| 1 | Exact version/schema/expected account/Device and nonzero original claim nonce. |
| 2, 3 | Exact secret widths; restored signing/encryption publics match saved certificate/owner. |
| 4, 5 | Root-signed certificate/own authority, exact encoding/account binding, matching saved Device secrets. |
| 6 | Global own authored sequence prefix and exact next sequence. |
| 7 | Root-signed credential encoding/account, hash-to-path, registered certificate; historical O/A/R uses verify role/ref and source authorization snapshot. |
| 8 | Spec encoding, two-account role election, membership Root signature/C/accounts and both Root consents. |
| 9 | Head shape/C/role/certificate, phase/generations/selected controls and sealed Native blob binding. |
| 10 | Original nonce/self-HPKE/Native hash and public restore/config/curve/OTK/SessionKeys/SID; zero fresh/packet calls. |
| 11 | Actual immutable Event ID/Device signature, C/D/E/SID/role/n/kind/Core/header/sequence/parents; Guard/fold and lane cardinalities. |
| 12 | Original projection self-HPKE info/AAD and text/request-to-Core binding; no replacement of sealed source. |
| 13 | Queue self-HPKE, C/request16/UTF8, uniqueness, no materialized request mapping and legacy reservation limits. |
| 14, 15 | Signed pending Event scope/ordinal; exact saved O/A refs, ordinary/head/quarantine capture and charge, no readable overlap. |
| 16 | Exact lane ordinal-to-Event mapping, Guard prefix and source bijection/cardinality. |
| 17 | Actual receipt, opposite Device/auth signature and exact event/lane/n/epoch binding. |
| 18 | Ack-intent state and reference to that actual receipt. |
| 19 | Exact outbox Event/status; Delivered iff actual saved opposite receipt. |
| 20 | Exact immutable request-to-Event mapping matches Core. |
| 21 | Hash/path and selected Offer/Grant membership; signatures, scope, transcript and epoch checked against actual Head/Native. No unexplained extra control. |
| 22 | Ready Device signature and exact phase/D/E/SID/genesis/role/suite body. |
| 23 | Static descriptor signature/C/expected role/certificate and canonical bounds; expiry is not a migration refusal. |
| 24 | Current saved consent: both Root signatures and C/certificate/suite binding, no wall-clock admission. |
| 25 | Initial pair: both Hello signatures/roles/credentials/membership/consent agreement, transcript and both Device Proofs. |
| 26 | Same static initial-pair decoder for the saved current-auth transcript; no current-channel lease inferred. |
| 27 | Known fork-reason envelope/caps, signed Offer/Grant/Ready/Event proof and role/scope checks; Ready-genesis old proof may be actual peer genesis. Evidence is retained, not replayed. |
| 28 | Exact global signer/sequence-to-real-readable-or-pending Event membership, no orphan authored rows. |
| 29, 30 | Exact `ORDERED_ACCOUNT_FENCE` and original zero32 pickle sentinel. |
| 31 | Root signature/exact encoding, account-to-path and registered-opposite-account binding; revocations remain archival. |

All kinds first pass the bounded primary index MAC, per-row AEAD/key/path/length/
hash membership, cardinality and inventory checks. Unknown types/paths/contexts
are not silently archived. This is source verification for S5.1, not permission
to start live indexed messaging.

## R3: final-source verification and executable provenance

Before edits, the complete submission-1 contributor ledger was copied to
[contributing-cases-submission-1.json](long-lived-dialog-a3-migration-s5-storage-evidence/contributing-cases-submission-1.json),
SHA256 `91f20a90adb1318afaa3795128f99a6b35ecf02e756056b7bf19823aac006d16`.
Every old contributor, including the original postprocess failure annotation,
remains in the updated cumulative ledger. Old report/manifest/per-case/static
files were not rewritten. The runner now retains those ledger-only annotations
and times through log/inventory/ledger aggregation; only final timing serialization
is outside its stopwatch. Cached compilation/setup/copies/recovery/cleanup and
all contributing attempts remain charged to the proper set.

The stopped F-feasibility-01 seed was validated after the reported power loss by
the supported path on new isolated copies of both sides. Exact source IDs remain:

- one: `82cbca9e0ab18b9fdff10615b8882d6a0711a8b126a42af900026da94b5e4b06`;
- two: `05259d8a06cf838363ea8e663fd28cc8e157ea14b8795e0b24e8c315729dd80e`.

Each has 56 entries, phase3, readable3/3 including genesis, four receipts and two
own Delivered entries, with actual imported Native/fold. Raw original DB hashes
are checked before/after copying; original DBs are never opened or repaired. The
source test explicitly pins this stopped feasibility seed. A future fresh seed
needs its own charged origin/provenance pins; the process group creates its own
fresh real origins. Accepted historical S1/S2/P/F databases are not used.

`stage-migration-s5-images.ps1` now records the baseline, exact effective Cargo
arguments, build log, profile/priority, and 439 non-ignored workspace source/Cargo/
toolchain/config/build-wrapper hashes before and after build/staging. It refuses
reuse of evidence names, changed source sets/content, live fixed-image owners or
changed frozen images, and verifies both copies against the just-built CLI.

Final fixed CLI/control-peer image: **67,716,096 bytes**, SHA256
`a92c85cad15d16b104bd947edeb18a551dfc45c032fea7d9a6abdbf7e1613ca7`.
The [final build/stage record](long-lived-dialog-a3-migration-s5-storage-evidence/static-field-images-r2-02.json)
binds it to `cargo build --jobs 12 --locked --offline --profile verification -p
kilogram-cli --bin kilogram-cli`, with equal before/after source sets/content and
both copies equal to that just-built binary. d9b/S3/exact S4/relay image hashes
remain unchanged. The current control peer is the same public ALPN1 actor as in
submission1, not an alleged predecessor image.

| Revision case | Whole runner seconds | Result and evidence |
|---|---:|---|
| C-ordered-recovery-r2-02 | 27.2427722 | 4/4: genuine prepared-v2 oversize-index refusal plus valid recovery; existing atomic/fenced/relabel/reservation controls. |
| C-storage-invariants-r2-02 | 14.6817899 | 2/2 on final records/preview/owner sources: canonical closed codec; budget payload/owner/summary/missing leaf, no cross-C backing payment, scratch membership and no second endowment. |
| F-storage-final-r2-02 | 76.1118936 | 7/7: both coherent archive substitutions before Fence/resumed Ready, expired/revoked positive archive, exact source/Native/fold/history, quota/CAS/IO poison/reopen and scoped decoder negatives. |
| F-process-final-r2-01 | 87.1234110 | Full five-scenario attempt: renewal/pending/debt, five actual partial boundaries and real two-C escrow/history passed. Capacity/version and six-kill scenarios stopped at a missing fixture-path precondition. Failed attempt retained and charged. |
| F-process-controls-final-r2-02 | 94.0231440 | 2/2 complete capacity/version/predecessor controls and six parent-kill boundaries with the correctly supplied validated seed. Same final source/image hashes; the three already-passing scenarios are explicitly skipped. |

All **five distinct process scenarios** therefore complete on the same final
candidate, across the full attempt and the focused completion. This is not a claim
that the first aggregate attempt passed. The missing `FixtureDirectory` argument
was an executor invocation error, not a product defect. The focused runner is a
saved [exact script text](long-lived-dialog-a3-migration-s5-storage-evidence/F-process-controls-final-r2-02-runner.txt)
derived from the unchanged case runner, using the same stable wrapper plus three
explicit skip names and the correct fixture directory. No product/test source was
changed between those two process attempts. No passed field scenario was repeated.
The second attempt's Rust tests took 78.29 seconds; the charged 94.023144 includes
the complete wrapper/setup/hash/cache/cleanup work.

The focused controls include actual staged/committed capacity outcomes, exact
retry/stale refusal, correct-owner positive controls and live/cold d9b/S3/exact S4
Busy/version fences. All six killed parents were observed alive and then killed
and waited. Observed private-byte samples for these small fixtures were about
9.5–10.4 MB; they are observations for these runs, not a large-profile peak claim.

Strict Clippy passed the affected state/CLI packages with `--all-targets` and the
exact literal `-- -D warnings` forwarding; see
[static-clippy-r2-03.json](long-lived-dialog-a3-migration-s5-storage-evidence/static-clippy-r2-03.json)
and its log. The previously passing ratchet Clippy evidence is unchanged alongside
the unchanged ratchet source. Locked/offline verification build and compile-only
CLI harness preparation passed. Final fmt and diff checks pass. All actual tests
ran through the stable Windows wrapper, Jobs12/BelowNormal and fixed paths.

Cumulative [contributor ledger](long-lived-dialog-a3-migration-s5-storage-evidence/contributing-cases.json):
**C = 279.3257298/600 s; F = 1181.4272386/1200 s; P = 0/2400 s**.
Remaining F is **18.5727614 s**. The sum retains submission1, the post-power-loss
seed check (24.3522197 s), intermediate successful revisions and the failed process
attempt. Static-only builds/lint/format/hash work is separate. No further
contributing run is attempted; a later long/whole-S5 run needs a root budget decision.

[Final source correspondence](long-lived-dialog-a3-migration-s5-storage-evidence/final-source-correspondence-revision-2.json)
shows zero current-file differences for all five final case snapshots and all 439
final build inputs. The C tests do not use the incidental field EXEs listed in
their snapshots; both final F process attempts and final owner case bind the
final fixed image above. There is no stale-binary claim based on the normal Cargo
main executable changing after test compilation.
[Validation](long-lived-dialog-a3-migration-s5-storage-evidence/validation-revision-2.json)
and [preservation checks](long-lived-dialog-a3-migration-s5-storage-evidence/protected-files-revision-2.json)
record exact-source coverage, recomputed budgets, an empty index and no remaining
cargo/rustc/kilogram process. All 124 protected prior files match.

For a further authorized reproduction, use a new case ID and evidence name.
Build/stage via `scripts/stage-migration-s5-images.ps1`; prepare harnesses with
Cargo `--no-run`, then use `scripts/invoke-migration-s5-case.ps1` with
`-FixtureDirectory .tmp/migration-s5-storage/feasibility-01` for owner/core **and**
the full `migration_s5_process_` group. The exact seed pins above apply to that
existing stopped fixture. The saved focused runner documents the two-test
completion; it does not replace a future explicitly budgeted full-group run.

## Reuse, preservation and review boundary

The final source correspondence records actual path/hash comparisons, including
the older invariant case's changed records/preview files. That older invariant
case is not the final ledger evidence; the two tests are rerun above.
Unchanged shared regressions are reused only with these branch arguments:

- `C-runtime6-regression-01` (8/8) and `C-component-regression-01` (16/16): the
  indexed layout/codec/append/page/allocator/runtime/ledger sources and their
  tests match those passing snapshots. The R1 change is the V4 ordered metadata
  dispatch/recovery path; fresh Component5/Runtime6 use `read_root`/indexed anchor
  loading directly. Shared `decode_generation` preserves the exact old validation
  body. Affected V4 recovery and actual predecessor/version fences are rerun.
- `C-native-regression-01` (1/1, real duplex through eight DH changes): ratchet,
  ordered_v2 and protocol sources match the passing snapshot. R2 adds static CLI
  archive checks and does not alter Native constructors, packet crypto or import.

The source comparison explicitly lists changed CLI verification/tests, V4 helper
and validation scripts rather than describing the whole worktree as unchanged.
No 100k, full511, whole P5 or extra two-context repetition is run. Strict Clippy
failures in newly added test convenience calls and the earlier compile diagnostics
remain as separate static logs; final passing logs supersede them without erasure.

The four root-owned assignment/navigation files, root review1, accepted contract/
reviews/manifests/provenance and protected development-environment file retain
their original raw hashes. Submission-1 report/manifest and all prior evidence
remain reviewable. The new candidate inventory supplies exact raw sizes/SHA256
and normalized Git blobs, excluding its own checksum cycle.

Ready continues to mean storage migration readiness. Live indexed runtime,
L1 runtime handoff/ALPN transition and whole-S5 feasibility remain for later root
assignments. HEAD and the empty index are retained; no commit/push is performed.
Stop for implementation review and an explicit accepted publication assignment.
