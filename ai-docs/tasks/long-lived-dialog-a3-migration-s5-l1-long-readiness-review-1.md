# A3-S5.2 first-long readiness review 1

2026-10-26. Root review of the concrete first-long readiness package, under
the user's standing authorization for review and correction cycles.

## Decision

First-long execution is not yet accepted. One controller-only correction is
required before the origin go-ahead. This is a possible false positive in the
verification procedure, not evidence of a product defect or an executed runtime
failure. No additional contributing C/F attempt is authorized by this review.

Reviewed inputs:

- `long-lived-dialog-a3-migration-s5-l1-long-readiness.md`, 13088 bytes,
  SHA256 `220b3cc1a74af235c6979b17c4e0ea2757f72aa2c8b1b6c758287beaae89b63a`.
- `long-lived-dialog-a3-migration-s5-l1-evidence/S-long-readiness-binding.json`,
  22114 bytes, SHA256
  `9c2e3ccb053169a60a249074397521f7284bd73dfbefa5e3def276a89f1057cc`.
- `apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs`, 20858 bytes,
  SHA256 `feaf65e3e6698ff56d5fad69c89f5d5423efb22f6d44b8da30c299e45000bf61`.
- Runner SHA256
  `c9c6841213181af75f3364128cf7d0697d45db5a7a8bf5bfda1b58986c3211ed`;
  ledger SHA256
  `5218356ef3d08914f1ce1c839da558a519a6ff33455f2635b7345f36a7b725c5`.

## R1: prove fresh both-indexed reconnect before checking zero replay

At exhausted controller lines 447-459, the final restart/import is followed by
`Node::wait` and zero packet/constructor assertions. The shared helper's
`reached` predicate (migration_s5_l1.rs lines 273-283) checks only saved frontiers,
debt and queue. Both peers already satisfy these values before fresh admission.
The test could therefore pass this phase without a new connection, and could
sample counters before cached bootstrap controls execute.

Both current leases alone are also insufficient for the replay assertion:
runtime7/network.rs lines 673-676 runs A's `handshake_a` after admission and before
the worker. A successful fresh A Resume round proves that this handshake has
completed. The earlier indexed-A/frozen-B B17 exchange proves a different phase
and does not close the final indexed/indexed reconnect obligation.

Make a narrow change local to the exhausted controller:

- Enable the existing diagnostic `trace_rpc` schedule for the final newly
  started indexed A, with no packet mutation, hold, fault or altered scheduling.
  Use that invocation's fresh log, not any previous process log.
- Within one explicit bounded wait, require both processes alive, both relevant
  contexts currently leased (and fresh admission evidence), and an exact
  successful A Resume completion from the fresh log: kind 9, stage finished,
  result reply9. Use the actual trace format; do not match a begin, write,
  failed response, previous log, or unanchored substring as success.
- Read fresh A/B status after the barrier. Only then assert the expected
  frontiers/debt/queue, unchanged Session/identity and zero packet/constructor
  replay. Retain subsequent refusal, exact retry, conflict, history and receipt
  assertions. Persist the public barrier evidence and its log provenance in
  the future result so the completed phase can be independently reviewed.
- Use existing current lease/locator/admission status to bind the observations
  to these two restarted owners. Preserve the proposed work120/whole140 outer
  policy; no unbounded wait or hidden increase in the contributing allowance.

Do not change the production protocol or relax shared wait semantics globally
to address this local test requirement. A controller-only compile does not
require a new production image. If the minimal approach cannot establish the
barrier with existing public/diagnostic observability, report the concrete
limitation before widening scope.

## Other reviewed scope and permitted preparation

Independent read-only protocol and state reviews found no other blocking issue
in their assigned scope: genuine original lineage, raw-only verification of the
closed originals, exact locally compared Native/identity material, authentic B17
receive and receipt, bounded exact 527/528-row history, old frozen cursor suffix,
and stopped checkpoint with its original absolute pinned paths. These are source
review findings, not claims that origin or boundary has run.

The original package's matcher record contains the two actual immutable logs
and nine negative text vectors, with expected results; its scope is static
parser verification. Ready02 and capacity01 retain their historical whole FAIL
records and the exact limited acceptance from continuation reviews 3 and 4.

Before editing, preserve the original controller bytes as a separate evidence
snapshot with a non-compiled extension. Keep the original readiness document,
binding, compiled-harness provenance and all historical evidence immutable.
Produce a revision-2 readiness document and a new binding, explicitly recording
this controller delta and the superseded compile. Preserve current product
images and the 33 ordinary production inputs.

Run the appropriate compile-only/static checks for the changed controller,
through the existing locked/offline stable-harness preparation workflow, Jobs12
and BelowNormal. No test execution, new contributing C/F attempt, new origin,
product image restaging, commit/push or cut3 is authorized here. Return the
concrete corrected source/static/harness hashes and callback for root review.

Actual accounting remains C147.8846192/F1444.3918456/P0; ceilings C500/F2900/P0.
F remaining1455.6081544 and the separate long reserve1215 are unchanged.
Only a later root approval file bound to the accepted runner and current ledger
can authorize the first origin. Boundary will still require its own decision
after the actual origin result and accounting are reviewed.
