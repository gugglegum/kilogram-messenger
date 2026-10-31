# Cold-A attempt01: failed outcome reviewed; evidence retained

2026-10-31. Root inspected actual app execution exec-ac1707ef-c2bf-43ee-96d8-023e4c936bb1: failed/exit1/45312ms, output complete. Root031dce verified raw three reports/run hashes, absence of P2/Parent, shared identity/clock/D1, started-to-claim binding, failed normal gate, cleanup-only reap and18 current public source pins. No private post-return access. Run9476/0f1705163e53d6cfa42ebd857d9eb10e6eccc06d116358601cd1ddb3dee0db33 is immutable actual evidence.

Result FAILED_OR_UNKNOWN/work_expired; raw preservation NOT established. Normal waits1422 false/0 true. Supervisor killed retained child and reaped it in cleanup (exit-1;3 waits,2 false/1 true), bothEOF; reported control-wrapper cleanup known. Missing P2/Parent means Core/owner/guard cleanup, comparison phases/equality/receipts/input release remain unknown. Current unchanged public source pins are not a substitute for missing in-run source proof. Sampled handles14513/WS230580224 and362 samples do not establish private progress.

Close F60 reservation: request11:40:37.495Z to final11:41:33.285Z =55.790s charged,4.210s released. S5.3 C1940.8824909/F1618.0782567/P0; combined C2528.3311018/F6334.3984056/P0; ceilings2630/1642/0; active0/inactive C689.1175091/F23.9217433/P0. Attempt01 consumed; never overwrite/rerun its identity. Standing user test-data authorization permits necessary later reviewed attempts without another per-run permission question.

Root source finding f163a2/fb2455: Parent is assigned work45/cleanup50 and writes final P2/Parent after disposal, but Supervisor immediately Kill()s a still-live child when its work45 loop expires, before waiting until52. Thus the advertised cooperative cleanup/report window is cut short by the Supervisor. This is a concrete control-flow defect in timeout handling and a plausible reason for missing diagnostic reports. It does NOT identify the phase/cause of the original45s overrun or prove that cooperative cleanup would have completed. Next contract slice should preserve failure status, allow a bounded cooperative cleanup/report interval before force, keep outer60/guardian bounds, and expose sufficient bounded aggregate phase evidence if needed. No blind timeout increase or synthetic comparison-matrix rerun.

Accept only faithful preservation/publication of this failed evidence, not product acceptance or successful S5.3. Publish exact12 paths below on parent064021611e95d30e4196f0ed4dfa7ad1c7726902. No product/source changes, private receipts or root navigation in this commit. Then perform a short source/public-evidence-only diagnosis and next-attempt contract proposal; no runtime or source implementation before review.

- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.D1.draft.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.native-plan.draft.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.private-preparation-review.md`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.preparation-accepted.md`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.activation.md`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.D1.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.started.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.claim.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.supervisor.json`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.run.md`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-cold-a-01-attempt-01.actual-review.md`
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/S5.3-standing-test-run-authorization.md`
