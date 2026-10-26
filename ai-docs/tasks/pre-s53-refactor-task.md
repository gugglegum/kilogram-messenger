# Refactoring before S5.3: assess, implement, review and publish

2026-10-26. The human explicitly agreed in the root chat:
"Да, я согласен с твоим предложением сделать оценку и провести рефакторинг перед S5.3."
This changes the next-stage order: complete S5.2 publication, assess and perform
a bounded refactor, review/publish it, then resume the queued S5.3 work.
Use the existing GPT-6.1 Sol Max executor without Fast.

## Verified baseline and stage boundary

S5.2 is published as `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`, sole parent
`73c499818ba9b699c331aa842e64e8432e90c59a`. Root independently verified all520
committed paths and normalized blobs against the accepted candidate plus its
acceptance document, unchanged raw working files, empty index and the preserved
excluded S5.3 task. GitHub compare of this SHA to master returned identical,
ahead0/behind0. Existing S5.2 acceptance/evidence is immutable.

The user's new order supersedes the immediate S5.3 activation paragraphs in the
queued handoff task and S5.2 acceptance. Preserve that queued task's existing
content/hash as historical context; this assignment is the explicit override.
S5.3 implementation, handoff tests and whole-P remain unstarted.

## First deliverable: a concrete assessment and small refactor plan

Write `ai-docs/tasks/pre-s53-refactor-plan.md`. This first dispatch is source and
document analysis only: no product/test/script edits, build, executable test,
fixture traversal, DB open, migration, state copy or new C/F/P contribution.
Use existing source, reports and measured evidence; do not repeat the entire
architecture audit or start a general cleanup project.

Inspect the areas implicated by actual work, separating active production paths,
compatibility code, test infrastructure and deferred components:

1. `apps/kilogram-cli/src/main.rs` has43675 lines at this baseline, with its main
   test module beginning at35750. Determine the relevant command/orchestration
   boundaries and which extraction would simplify upcoming work. Size alone is
   not a defect; avoid mass movement of dormant code without a concrete benefit.
2. L1 validation spans the older Engine and the indexed runtime/storage path.
   Review the real R1 own-F, R2 debt128 and R3 Ready corrections. Identify genuinely
   identical, side-effect-free rules that could be shared or represented by safer
   types; keep version-specific permissions, formats, persistence and transition
   semantics separate. Do not merge incompatible old/new runtimes for cosmetic
   similarity or weaken an independent storage guard.
3. Long-test controllers and finalization lost detailed RAM-only output after a
   late preservation failure. The actual phase-writer bug is already fixed and
   tested. Identify a small improvement to result lifetime or ownership only if
   useful beyond that fix. Preserve failed-attempt accounting, exact provenance,
   bounded deadlines and stable executable paths. The recent opaque test kernel
   took0.88s while its whole runner took24.1463993s; preservation took46.26s inside
   whole67.5439277s. Treat the difference as measured combined overhead, not proof
   that a particular diagnostic is the bottleneck.

Choose preferably one or two bounded changes, with a clear reason each helps
the next development steps. Other findings may become a ranked later list.
For each chosen change provide:

- exact current files/functions and the observed duplication, coupling or failure
  mode; distinguish established evidence from a hypothesis;
- the intended module/API boundary and how ownership and permitted state changes
  become easier to review; no pattern catalogue or speculative general framework;
- the behavioral equivalence obligation, affected callers and compatibility paths,
  and which original security/storage invariants must remain identical;
- a small before/after structural criterion: fewer independent implementations of
  one rule, a separated command adapter, or durable results before a fallible tail;
- exact focused verification, justified reuse of unchanged evidence and a bounded
  new C/F budget proposal. Separately recorded compile-only/Clippy/fmt may be static;
  actual setup, fixtures, waits, failures and cleanup remain charged contributions;
- implementation order and rollback boundaries. Keep mechanical movement distinct
  from changed semantics. Do not turn optimization or protocol redesign into a
  supposedly behavior-preserving refactor.

Assess maintenance benefit against review and verification cost. Do not require a
new public API, crate, class hierarchy or framework merely to make the code look
uniform. Prefer changes that reduce the opportunities for the already observed
mistakes and make S5.3 easier to implement safely.

## Invariants and evidence

Wire/storage formats, cryptographic primitives, identity/Native/session lineage,
authorization ordering, current final gates, atomic transitions, credits/debt,
limits and old/new compatibility retain their accepted semantics. Any proposed
semantic correction needs an explicit separate finding and root decision.

Do not alter published S5.2 artifacts or relabel historical FAIL/split results.
Its actual C172.0310185/F3331.2897354/P0 and earlier S5.1 accounting stay immutable.
No new runtime budget is activated by this assessment. Do not reuse the remaining
S5.2 budget silently. New implementation verification will receive a distinct
explicit allocation after root reviews the plan.

The genuine exhausted source NCjBHb and private backup remain stopped/inactive at
their exact pinned absolute paths. No concurrent copy of a Native domain, secret
export, automatic recovery or fixture activation is permitted by this assessment.
No unchanged bulk100k suite or regeneration of511 messages is warranted merely
for module extraction. Later Windows checks use locked/offline verification,
Jobs12/BelowNormal and `scripts/run-cargo-tests-stable.ps1` with fixed EXE paths.

## Review, execution and publication

Callback root with the assessment/plan path, raw hash, exact baseline, ranked
findings, chosen scope and verification allocation, then stop before edits.
The user has already authorized the refactoring stage; root resolves technical
scope and performs review/fix cycles without asking the user to copy messages
or repeat permission. Root will dispatch implementation after plan acceptance.

Implement each accepted logical change, submit its evidence and exact file/blob
allowlist, and stop for root review. After acceptance root assigns ordinary
commit/push, independently verifies the publication and proceeds to the next
accepted logical change. No broad add/reset/amend/force or unrelated modifications.
After the bounded refactor is accepted and published, S5.3 mapping resumes against
that actual new baseline, retaining the accepted protocol contract and checkpoint.
