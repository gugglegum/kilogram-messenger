# RFC-0202: M1.1.75 instrumented decoder smoke evidence

Status: accepted locally; sustained campaign remains pending.

## Decision

M1.1.75 records the first clean-source instrumented run of
`canonical_sync_v2_decoders` using the pinned M1.1.74 toolchain. The run is
accepted as a smoke and coverage-pipeline gate only. It is not accepted as the
eight-hour sustained campaign required before candidate transport activation.

The tracked evidence contract is `CANONICAL-SYNC-V2-FUZZ-SMOKE.json`. Large
raw logs, the evolving corpus, and the binary profile remain ignored build
evidence. Their exact paths, sizes, and SHA-256 digests are recorded in the
tracked manifest so the originating host can validate them without adding
large generated files to Git.

## Accepted observation

- source revision before and after:
  `f9148093a7476e2d77c4a967b018c4b26a230225`;
- worktree clean before and after: true;
- requested duration: 300 seconds;
- measured duration: 301 seconds;
- completed executions: 5,697,050;
- exact initial seed count: 20;
- final corpus files: 859;
- generated failure artifacts: 0;
- process exit code: 0;
- jobs and workers: one each;
- coverage profile generated: true.

`llvm-profdata show` accepted the generated profile and reported front-end
instrumentation, 871 functions, 1,986 blocks, and a total count of
838,948,912. This proves that the pinned runner, runtime, corpus reset,
feedback loop, and profile-production path work together on the recorded host.

## Closed boundary

`scripts/verify-kilogram-canonical-sync-v2-fuzz-smoke-boundary.ps1` rejects:

- a source revision mismatch or a dirty source claim;
- fewer than five minutes or fewer than one million executions;
- any generated failure artifact or non-zero exit code;
- missing profile generation or invalid profile metadata;
- more than one job or worker;
- a false claim that the sustained gate completed;
- any transport registration, dial, runtime activation, or authorization.

The boundary does not require ignored raw evidence to exist on every clone.
Its tracked manifest is the portable acceptance record. When raw evidence is
present on the originating host, its recorded digest remains independently
checkable.

## Remaining gate

The campaign contract still requires at least 28,800 seconds, at least
1,000,000 executions, profile generation, a clean unchanged committed source,
and zero generated failure artifacts. Only a new sustained record satisfying
the complete contract may close that gate.

M1.1.75 does not register or dial `kilogram/m1/sync/cbor/2`, authorize the
candidate transport, or change the active runtime.
