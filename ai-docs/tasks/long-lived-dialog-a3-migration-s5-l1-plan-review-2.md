# A3-S5.2 plan review 2 — accepted for implementation

2026-10-26. Root accepts the corrected S5.2 implementation mapping. This is plan
acceptance, not implementation acceptance, runtime PASS or permission to publish.
The separate [implementation assignment](long-lived-dialog-a3-migration-s5-l1-implementation-task.md)
authorizes product work and the explicitly bounded verification tranche below.

Baseline remains `73c499818ba9b699c331aa842e64e8432e90c59a`.
Reviewed inputs, preserved as historical documents:

- [Submitted plan](long-lived-dialog-a3-migration-s5-l1-plan.md): 38467 bytes,
  SHA256 `155387113ed699d4e1fa85e2314770dfccf1ff8f9271a748d7adf3ff027d23be`.
- [Review 1](long-lived-dialog-a3-migration-s5-l1-plan-review-1.md): 10004 bytes,
  SHA256 `36379adada4d7d4ad0c2d2c239a8f7b19cc5f8d0280b1bbf8b482e9732cf21dc`.
- [Revision 2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md): 25257 bytes,
  SHA256 `f6d6439bc0af594153d30d30a4bcac37188f4c6c50bdaec8c3e941ff170a865f`.
- [Accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md): SHA256
  `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`.

Revision 2 supersedes only the corrected portions identified in its introduction;
the original plan's approved state/owner/Native/history/credit mappings remain.
Root performed source/document review and a narrow independent follow-up with the
same GPT-6.1 Sol Max protocol reviewer. No build, test or fixture DB open occurred.
The candidate document hash, unchanged baseline, empty index and document-only
worktree scope were verified; whitespace checks passed.

## Findings closed

**R1: account-wide invalidation.** Revision 2 section 1 computes every affected
occupied context by account membership, independent of own/peer, connectivity or
enabled state. Pin advancement atomically changes relevant policy state and closes
all old affected leases/cache before another dispatch or final Native commit.
Per-C tokens preserve independent unrelated-context operation. Exact same-revision
retry, conflicting same revision and rollback semantics are explicit. Shared-peer
two-C and different-peer isolation assertions are part of the existing matrix.

**R2: usable same-C renewal.** Section 2 supplies actual offline propose/cosign
commands, exact original certificate tuple/membership, one local Root per process,
current signed authority inputs/floors and a legacy-compatible completed invitation
plus raw Consent1. Historical signature/scope verification is distinct from current
permission. The independent reviewer checked the frozen-era `f59d285d` command
source: stopped authority-update followed by completed context-import is available
and preserves the existing Spec/Native. The plan correctly states that the old
context importer alone does not reject all equal-revision byte conflicts; exact
bound snapshots must first pass its stronger authority-update path. The public
producer and both import branches still need actual implementation/runtime proof.

**IPC representation.** Section 3 uses the existing uint/bytes/array algebra,
canonical `[]`/`[bytes]` options, exact tuple arities/caps and new tag11, without
changing frozen wire1/IPC2. Tag35 wraps a required bounded JSON byte string using
the existing strict five-field MigrationConfig. Root checked the existing config
parser and held-core capacity validation. Required fields, duplicate/unknown fields,
path ownership, headroom and config CAS remain explicit implementation checks.

**Accounting.** Separately executed compile-only/build/stage/Clippy/fmt are recorded
as static-only. Preparation inside a contributing runner is charged in that runner;
real fixture generation is always contributing work. No double charge or free setup.

## Decisions activated by the implementation assignment

Use one genuine exhausted origin **A511/B16 Text**, preserving its actual opposite
cut and stopped Native domain for later handoff. No duplex511 alternative is granted.

New S5.2 tranche: **C <= 500 seconds, F <= 1800 seconds, P = 0**. Published S5.1
actuals remain C279.3257298/F1236.2886628/P0. The combined maxima are therefore
C779.3257298/F3036.2886628/P0; this is an explicit new allocation, not a reset.
Later C350–550/F650–1100 remain estimates only. Whole P is separately gated and
still requires one continuous run within2400 seconds; no fragments substitute.

Small gates precede the long origin. Before generation, require the remaining
850-second seed upper bound, 140-second boundary gate and contingency to fit.
The whole seed attempt has a900-second stop including cleanup. Failed attempts
stay charged; retry or budget amendment needs root review before more execution.
Do not claim a later handoff/Event2 boundary from this L1-only cut.

The separate assignment now permits implementation and targeted verification.
The executor must submit the final source-bound candidate and evidence to root,
then stop before commit/push. Publication is assigned only after implementation
acceptance, under the user's standing authorization.
