# P11A1 normal lifecycle primitives: root acceptance

Date: 2026-11-02. Baseline: `f70aec67a304354e6130198d48020b448951376e`.

Accepted scope: the nine dormant test-support source files and the implementation/evidence package in [the executor report](long-lived-dialog-a3-migration-s5-final-block3e-p11a1-lifecycle-implementation.md). This accepts source, static checks and focused pure tests. Actual shutdown, stopped-source reading, relay finalization and the composed P11 scenario remain unexecuted.

## Source review

- A normal stop retains the original source/PID/incarnation and the original minimum of 15 seconds and the whole deadline before requesting shutdown. A successful acknowledgement does not count as process termination. Only requested exit code zero is expected; both actual waits precede either source read.
- Decoder final records retain the actual final counters and original stream identity. Normal closure checks EOF, absence of a partial record, the already-closed operation ledger, actual source deltas and the last observed Native commit. It does not call finish twice or synthesize a missing packet. Both pending outcomes remain unresolved if either owner's closure validation fails.
- Reopening requires reconciliation. Closed-stream summaries preserve actual attempts, successes and aborts, including final adjustments; they do not replace the future whole-scenario accounting predicate.
- Normal relay finalization requires both node owners reconciled and their state locks available. It publishes the stop request, waits on the original child with the original bounded deadline and accepts only exit zero without forced cleanup. Drop cleanup retains an unresolved outcome when normal completion was not proved.
- Existing killed-process reconciliation and live-only incarnation guards remain distinct and protected. No product protocol or cryptographic behavior changes are included.

## Independent validation

Root verification `adac36` matched 663 distinct file pins, all 39 executor candidate paths (394441 raw bytes), nine source blobs (114314 bytes), 571 current compiler inputs and 501 compiler units. Compile and pure-run captures agree; all pure-run units are fresh and the frozen guard passed. Normal CLI and fixed field images retain their recorded hashes.

The actual executor command records were compared with all six receipts: three Clippy passes, one compile-only pass, one focused pure pass and format check. The final pure log contains exactly 20 expected passing names (8 new, 12 affected), zero failed or ignored selected tests. Current source diff whitespace checks passed. The index is empty and unrelated navigation changes are preserved. No redundant test run was added by root.

The first packaging filename error remains recorded in the executor evidence. It did not require source repair or a Rust test rerun. [Root verification](long-lived-dialog-a3-migration-s5-final-block3e-p11a1-evidence/root-verification.json) preserves the independent checks and actual application command identities.

## Publication and continuation

Publish only the exact separately supplied P11A1 allowlist, on the baseline above, with ordinary commit/push. The allowlist also includes the previously reviewed P11 premises, contract, contract grouping review and P11A1 task so their references become durable. Exclude navigation files and administrative publication receipts/manifests.

After callback, root verifies the committed parent, exact paths/blobs and remote branch before assigning the next small P11 slice. Remaining work includes the bounded terminal source/receipt/Native reader, same-route restart and final composition, memory observation, sanitization before durable status logging, and runner provenance/activation. Neither this acceptance nor publication establishes whole S5.4 runtime feasibility. S5.3 remains closed.
