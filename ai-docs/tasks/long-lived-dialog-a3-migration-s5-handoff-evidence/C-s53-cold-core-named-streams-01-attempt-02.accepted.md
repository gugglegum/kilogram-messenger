# Accepted C2-F named-stream refusals, attempt-02

2026-10-31. Root accepts exactly the two synthetic native file/directory named-stream refusal cases. The prior attempt-01 remains failed. Whole cold preservation, private access, G1 and S5.3 remain open.

The [source review](C-s53-cold-core-named-streams-01-attempt-02.source-review.md) was independently verified: old Core snapshot is byte-exact; current Core differs only in two test-family bindings; controllers differ only in those bindings/hashes and the three-byte array fix already covered by the [92-case regression](oracle-cardinality-r75-accepted.md). Source gates1b86fb/4e565b reused unchanged native lifecycle and assertions.

One native invocation returned exit0. The [worker report](C-s53-cold-core-named-streams-01-attempt-02.cases.json) and [supervisor report](C-s53-cold-core-named-streams-01-attempt-02.watchdog.json) both observed two cases:
- File: actual named_stream, native0, bounded_refusal, capture_s/s; locks6/acquired2; peak handles29, record12085392, path6920, inventory3578.
- Directory: actual named_stream, native0, bounded_refusal, locks/s_one_lock; locks2/acquired0; peak handles15, record12079216, path3016, inventory0.

Both hooks had no failure. Root independently checked every pre/final Core field (38 each), intended exception chains/marks, helper and premise facts, closure before Core, phase order and final ownership. Both controllers' original 68-field helper assertions passed. Geometry31directories/26files/113checks; default7384/named2 bytes. Fresh five types and three explicit initializers, no compilation diagnostic. Child43176 exited/reaped; both EOFs and managed/native owners closed as recorded; no forced/capped/unknown cleanup. This does not test native false-close paths or establish continuous namespace absence.

The [run record](C-s53-cold-core-named-streams-01-attempt-02.run.json) preserves actual args/item/return and source/report hashes; [started](C-s53-cold-core-named-streams-01-attempt-02.started.json) binds the one-use [allocation](C-s53-cold-core-named-streams-01-attempt-02.root-allocation.json). Root matched item exec-5f3ec167-62ca-4010-9d44-1cc4cf67cee5, exit0, nontruncated output,10 input pins and all complete reports (gate4cf72d). Three runtime reports total23060 bytes. Native wall4.9725413s, supervisor4.6822264s, worker4.0249924s and request-through-return8.313s overlap.

Reservation CLOSED: charge C8.313 once/release21.687. Actual S5.3 C1534.8714909/F1562.2882567/P0; combined C2122.3201018/F6278.6084056/P0; ceilings1560/1642/0; active0; inactive C25.1285091/F79.7117433/P0. The separate pure-r75 duration remains5.474s, not charged twice.

Publication scope is supporting scripts and this accepted synthetic evidence, including the unchanged RootNative dependency and preserved old Core. The rest of the shared S5.3 worktree is excluded. Publication does not authorize other helper entrypoints or original/private state access. Keep these sources/results immutable; future attempts need fresh output paths and a scoped decision.
