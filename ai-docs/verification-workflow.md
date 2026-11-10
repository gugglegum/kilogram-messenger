# Verification workflow

## Executor autonomy: user decision 2026-11-10

This section supersedes older task-level instructions that require an architect callback after every failed test, timeout, fresh attempt or routine implementation correction. The user explicitly requested complete executor-owned implementation/debug/test cycles to reduce architect usage. Historical failures and their original scopes remain immutable.

- The architect defines the task, acceptance criteria and security/architecture boundaries, then reviews the completed diff and evidence. The executor owns implementation, diagnosis, routine repairs, test preparation and justified reruns inside those boundaries. No separate architect turn is required merely because a command failed or timed out.
- Preserve failed output and use fresh result directories. A one-use runtime attempt remains one-use; a new attempt is allowed within the assigned workload without a new design/review round. Record new attempt identity, current source/image/input bindings and the reason for retry. Never overwrite old evidence, reuse consumed claims, relabel a failed attempt as successful or report only a lucky pass.
- A retry must test a correction or an explicit transient-failure hypothesis. Do not repeat a deterministic failure unchanged indefinitely. When no plausible in-scope fix or useful discriminating test remains, report a concise blocker with evidence and the decision needed. Do not hide failures to satisfy a request for a ready result.
- Keep finite watchdogs and verified owned-process cleanup for each run. The executor may correct development build/test watchdogs when evidence shows the harness allowance is unsuitable; record the reason and previous failure. Product latency/resource limits and acceptance deadlines are requirements, not adjustable just to obtain a pass. No aggregate one-turn work budget or arbitrary one-attempt limit by default.
- For assigned full-runtime work, the executor may perform mechanical fresh-attempt binding, preparation and activation steps on the architect's delegated authority, preserving the existing checks and one-use semantics. Record delegated issuance honestly. A literal root-only approval field or hardcoded attempt must not create an implicit policy exemption: adapt such orchestration plumbing within the assigned scope and report its diff, or identify a concrete contract conflict. Do not silently disable authorization or binding checks. User approval restrictions imposed by tools remain in force.
- Return to the architect with the ready result and relevant passing checks, a substantive architecture/security/scope decision, or a genuine blocker. Progress updates to the user are fine; routine status callbacks and permission loops are unnecessary. No polling between chats.
- Independent review before ordinary commit/push remains. No change to E2EE, durable history, recovery guarantees, real-data access, foreign-process protection or the stable Windows harness rule. Publication that invalidates a prepared inventory still requires fresh preparation; never restamp evidence.

Current application: the BG path/tail task may proceed from its concise internal design to the narrow path interoperability fix, bounded final-tail timing instrumentation and focused regression tests without a contract-only callback. Keep call order, all validation/freshness/security gates, final acceptance markers and original F/P deadlines unchanged. Removing/reusing security checks or changing path authorization semantics still requires an explicit architecture decision. A full F run belongs to the subsequent integrated task after source acceptance/publication; this is not a per-test retry gate.

Effective 2026-10-31. The user explicitly requested simplification and optimization without reducing security or reliability. This is the current coordination policy; historical task instructions and results remain recorded in their original scope.

## Work and review

- Define one observable result, its inputs, file scope and acceptance checks. Review a new protocol, storage, authority or destructive-data contract before implementation. Once that contract is accepted, ordinary implementation and safe focused unit tests belong to one work cycle, followed by independent review; do not require a fresh contract/readiness/allocation document for every helper correction.
- Preserve the existing S5.3 acceptance obligations. A blocker must identify a concrete threatened invariant, incorrect behavior or missing required evidence. Extra diagnostics or generality do not automatically become acceptance gates.
- Use a small current checklist, actual commands/results and one concise result per attempt. Preserve failures and enough source identity to reproduce a result. Reuse unchanged evidence within its proven scope. Review changed behavior and its dependencies rather than repeatedly reconstructing the entire history.
- Keep E2EE/authentication/authority, key lineage, lossless durable history, replay protection, crash recovery and product resource bounds. Keep separate explicit boundaries around original state, private keys and live network/process experiments. This policy grants no new private access.
- GPT-6.1 Sol High is the default for bounded implementation tasks; XHigh is for difficult diagnosis or independent security/concurrency review. Max needs a specific unresolved problem that benefits from escalation. No Fast mode or Astra executor. Assess time to accepted work and correction count, not first-response speed.
- After acceptance, the executor commits/pushes only the exact reviewed scope; the orchestrator verifies committed content and the remote. Unrelated dirty work remains untouched. S5.4 still follows S5.3 acceptance/publication.

## Pure public-data tests

Use this smaller execution profile only after review establishes that the test reads named public source/reports, evaluates bounded in-memory data and writes only its own public result. No keys, original databases, fixtures, network, product entrypoints, native helper compilation or subprocesses from the test worker.

A normal managed parent may start one fixed-path PowerShell worker, wait with a 30-second timeout, terminate its own child on expiry, and allow a bounded 5-second reap. Timeout, unexpected failure or unknown cleanup cannot be a pass. This is a development-test hang safeguard, not an atomic containment or real-time guarantee. Stronger containment remains required where the actual data/process risk requires it.

Use ordinary disposal, explicit result fields and a measured output cap. Preserve the original useful error and reject incomplete evidence. A symbolic proof of every possible serialized byte, independent parent self-kill guardian, UTC-to-QPC conversion and native Job helper are not required for this pure-data profile. Keep stdout/stderr small; use a result file for the bounded details.

Historical C/F/P accounting stays intact. A later root-approved pure-test run uses this profile with recorded elapsed time; do not engineer new watchdog layers to fit the old inactive work6/whole10 proposal. No run is activated by this document.

## Current oracle correction

The paused r75 files have never run and may be simplified in place. For this pure check only, this policy supersedes R74-A2/A3 and the symbolic-size-proof part of A1. Keep the two original-error localizations, all 92 matrix rows, both frozen function copies, the exact three-byte candidate, all 38 field assertions, intended negative rejection checks and synthetic-data labels. Preserve the old r73 sources and failed evidence unchanged.

First deliver the simplified source for one focused independent review of the data boundary and test assertions. After that review, execution and ordinary source/test fixes can share one bounded work cycle. No separate new contract series is needed. The larger native/private preservation and G1-G4 acceptance scopes remain open.
