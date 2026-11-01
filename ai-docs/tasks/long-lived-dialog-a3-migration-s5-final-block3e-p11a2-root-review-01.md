# P11A2 root acceptance

2026-11-02. Baseline `5fef1926d532d2a5b926739d71af7680818550e8`.

Accepted: the dormant authenticated same-route locator verifier/read seam and focused signed-fixture tests described in [the implementation report](long-lived-dialog-a3-migration-s5-final-block3e-p11a2-locators-implementation.md). No blocking source findings.

Root read both complete source diffs, the real protocol parser/signature verifier and the check scripts. Prior bytes are authenticated against the expected context, owner, peer and exact relay-only route; cached revision/endpoint must agree with signed content. Current bytes retain current-time validity checks. Lower revisions and equal revisions with changed bytes fail; valid newer same-route publications permit unchanged or changed endpoints. Expired historical prior evidence does not invalidate a valid newer replacement. The protocol parser independently enforces positive revision and a well-formed interval. No new publication-time 60-second margin is imposed at observation time.

The original strict P10 entry points remain strict. Four-publication scope/role/account/device binding and per-owner current endpoint equality are preserved. Current time is sampled for each record and Controller liveness/deadline is checked before return. The actual admission function body is unchanged. These source checks do not establish actual filesystem publication, restart, import, admission or liveness behavior.

Independent verification `95091e` checked 646 file pins, all25 candidate paths/raw285409B, two source blobs/raw25704B,571 current compiler inputs and501 units. Compile/pure captures match; all pure units are fresh and the frozen guard passed. Actual app commands and four receipts agree: strict Clippy, compile-only, targeted pure run and formatting all exited zero. Exactly10 tests passed (6 new and4 existing), zero selected failures/ignored. Scoped whitespace checks passed; index empty/navigation preserved. No redundant runtime/test run was performed. An initial root audit assumed an A1-only optional schema key; the corrected read-only audit is recorded in the verification file, with no source or evidence mutation.

[Independent verification](long-lived-dialog-a3-migration-s5-final-block3e-p11a2-evidence/root-verification.json) retains actual command identities, test names, binary/source/capture pins and check scope.

Publish only the exact separately supplied P11A2 allowlist, including its task, root review and root verification, by ordinary commit/push. Root must verify committed blobs and remote master on callback before assigning subsequent work. Terminal source/receipt/Native verification, complete restart composition, memory observation, pre-durable-log sanitization and final runner/provenance remain pending. Whole S5.4 feasibility is unexecuted; S5.3 remains closed.
