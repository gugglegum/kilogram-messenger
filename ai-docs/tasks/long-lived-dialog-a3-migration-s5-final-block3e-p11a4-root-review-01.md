# P11A4 root acceptance

2026-11-02. Baseline `6349b48e62d67faedc93a1ec99694cc12ff03049`.

Accepted: dormant bounded terminal graph and focused tests in [the implementation report](long-lived-dialog-a3-migration-s5-final-block3e-p11a4-graph-implementation.md). No blocking source findings. This proves properties of the supplied normalized inventory; actual database completeness, historical witness binding, Native state and runtime remain outside this acceptance.

Root read the complete graph and tests plus all existing-file diffs. The non-test constructor uses authenticated existing scopes and their verified selections, exact cuts and original genesis IDs. Insert validates bounded positions/scope/format/request and uniqueness before mutating indexes. Finish requires all1344 Text records, exact lane predecessors, actual same-context Text parents and an unblocked causal walk. The walk follows the current history rule: eligible lane heads, lowest EventID, advance only that lane. It detects cycles without assuming author sequence is a global clock. Canonical commitments cover all A3 Record fields, and typed comparison also covers original scope, counts, tails and deterministic order.

The large tests explicitly use synthetic normalized metadata; the four-record signed bridge exercises A3 output through the actual graph but does not authenticate an entire origin export or1344 signatures. The private synthetic constructor is cfg(test) only. Existing A3 verifier remains unchanged; shared fixture imports/registration retain their behavior.

Independent root check `1d53ae` verified708 pins, exact50 candidate paths/raw355406B, five source blobs/raw52061B,576 current compiler inputs and501 units. Final compile02/pure02 captures and artifacts agree; all pure units fresh/frozen guard passed. Actual application commands match all10 receipts. The final strict Clippy, compile-only, pure and format checks pass; exactly16 expected tests (8 new+8 affected) pass with zero selected failures/ignored. Failed Clippy revisions, the early missing-manifest wrapper failure and formatting failure remain preserved. The early pure wrapper executed no test harness. Scoped/all-source whitespace checks pass; index empty and navigation preserved. No additional tests were run by root. Two root read-only audit schema assumptions were corrected without changing implementation or executor evidence.

[Root verification](long-lived-dialog-a3-migration-s5-final-block3e-p11a4-evidence/root-verification.json) retains actual calls, tests and pins. Publish only the exact separately supplied allowlist, with task/review/root verification. Root verifies parent, committed paths/blobs and actual remote after callback.

For later checks, complete project-edition formatting before the final compile and require the compile capture to exist before launching its dependent pure run. This prevents the avoidable sequencing/rebuild failures observed here without changing acceptance requirements.

Remaining work: bounded actual repository traversal and absence checks; original profile/source/migration/Ready/controls and Legacy/P7/B640/R1/ACK bindings; independent actual owner/cut equality; Native checkpoint, restart composition, memory/sanitization and final runner/provenance. S5.3 remains closed; whole S5.4 feasibility remains unexecuted.
