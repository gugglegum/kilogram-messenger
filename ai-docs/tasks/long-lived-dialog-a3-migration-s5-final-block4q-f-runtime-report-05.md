# Block4Q: sole F05 Run failed after native rendezvous

2026-11-03. Exactly one published, externally activated F05 Run executed on HEAD `81a61ef3a8ef27cc77315258a2a59ec47e98b7bd`. Actual outer child and supervisory owner returned1. The retained public failure is **`harness_nonzero`**, with harness exit101; the launcher exited124 and internal runtime cleanup was forced. There is no assertion document or functional/completion marker. F05 is consumed/failed; no retry, activation refresh, state rewrite, source repair, build or WholeP work occurred. Whole S5.4 remains unaccepted.

The outer/native claim, original launch/capture, native clock request/reply and join/permit bindings are present and independently matched after the Run. This establishes the observed boundary reached, not functional completion. **No S54_ADMISSION_DIAGNOSTIC was retained**: both stream diagnostic counts are0. The saved public evidence does not establish the precise failing functional check/error; no raw arbitrary error was declassified, and no cause is inferred from the memory-observer abort file. Historical F03/F04 causes are not reinterpreted.

## Exact invocation and observations

Root run ID `1e886725b06c70d42b9b566c32ff8dc59008fa160a65b21c469c9a01106aedf1`; purpose `FeasibilityF`; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-05`; exact selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.

Inputs under `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-05/` remain unchanged:

- `inputs.json`:117091B, SHA256 `b90e60597daebed5d5938ffc4e09a8168c40b024dd5b37352ef97edcb176a58c`.
- Root-issued `activation.json`:1003B, SHA256 `a5bfc75cfd6566d3f78d5587893df99bb128d86bcd460c781fe1a3060b3db58d`; validity `[1793670252962,1793677452962)`. Remaining validity is not retry authorization.

Actual published Run child:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' -NoProfile -NonInteractive -File 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1' -Action Run -Purpose FeasibilityF -InventorySha256 b90e60597daebed5d5938ffc4e09a8168c40b024dd5b37352ef97edcb176a58c -ActivationSha256 a5bfc75cfd6566d3f78d5587893df99bb128d86bcd460c781fe1a3060b3db58d
```

Fresh administrative root `.tmp/s54-block4q-f-runtime-05`, adapted from the preserved M owner/preflight without altering historical scripts. The preflight used a real fresh installed pwsh child: all19 bound environment presence/hash rows matched; original owner PATH32afa634…, configured parent b27e0528…, actual child f2cb8ccd…; process-only regular Rust1.98 bin prepended once, offline true, BelowNormal, TOKIO_WORKER_THREADS absent/24 effective workers. It verified exact input/activation/current image/source pins and published HEAD, only inputs+activation run entries, successful native process inventory with zero exact-image conflicts, absent WholeP and sufficient validity. It did not repeat full inventory/compiler/matrix checks outside production.

| Administrative step | Actual tool launch → return | Actual child/owner exit | Elapsed seconds | Limits work/whole/cleanup |
| --- | --- | --- | --- | --- |
| preflight-01 | `8f143f` → `507164` | 0/0 | 11.5162076 | 270/300/30 |
| run-01 | `2a8be5` → `4e8688` | 1/1 | 128.2484486 | 570/600/30 |
| postcheck-01, read-only | `aeedfc` → `761832` | 0/0 | 9.5733325 | 570/600/30 |

Actual supervisorPID67120, outer childPID34704, launcherPID8784, harnessPID68712. All three administrative receipts verify handle/Job membership, waited actual child, bounded EOF, no reader failure and verified nonforced supervisory cleanup with after[]. Actual Run child exit and EOF observation ticks were captured **before** cleanup, pin reads and supplementary diagnosis. Runtime owner cleanup is a separate result and was forced, as detailed below.

## Original F clock and failure boundary

Original owned T0 is tick520536125, UTC Unix1793670443623 (2026-11-03T01:47:23.623Z). The published F limits remained controller450/work480/outer510 seconds from that T0:

| Observation/deadline | Tick milliseconds | Elapsed from original T0 |
| --- | --- | --- |
| controller cutoff | 520986125 | 450000ms |
| work cutoff | 521016125 | 480000ms |
| outer cutoff | 521046125 | 510000ms |
| durable checks observation | 520663781 | 127656ms |
| actual stdout/stderr EOF observed | 520663859 | 127734ms |
| actual outer child exit observed | 520663890 | 127765ms |

Supervisor observed EOF at01:49:31.3487348Z and child exit at01:49:31.3782781Z. Its longer570/600 allowance only captured/contained the original execution; it did not reset or extend F. `work_expired=false`; the public failure classification is harness_nonzero, not a diagnosed deadline expiry. Durable outer fields for individual cutoffs are absent on this failure record; the numeric cutoffs above are the retained original launch binding. Durable `actual_desktop_return=pending` is distinguished from the separately observed actual return1.

Five post-run binding checks passed without rerunning admission:

1. Outer claim matches actual outerPID34704, fixed identity and original inventory/activation pins.
2. Launch/capture bind actual owner/launcher, original T0 and exact450/480/510 tick relations.
3. Native claim matches actual owner/launcher, inventory/launch hashes, nonce and F05 identity.
4. Clock request equals that claim with the defined classification; reply binds its hash, original T0 and an unexpired original controller sample/remaining interval.
5. Exactly one `join-68712.json` and `permit-68712.json` agree on actual harness/owner/attempt/nonce, with membership false→true.

No assertion or successful final protocol layout exists. The published production success/final-tail path did not complete; supplementary pin preservation checks are not a substitute for that path.

## Public output and cleanup

`outer-result.json`:3756B, SHA256 `368555b29c2c3d893aa29e26e3630dff21399a6b90928b19d39ba7343cb97902`; `outer_checks=failed`, `failure=harness_nonzero`, `whole_attempt_verdict=not_established`.

Filtered original protocol pins:

- stdout214B, SHA256 `f5263e5f6749e8afdd28976df3df4506751cf9c93790b90ad017235f6c6ffe4e`.
- stderr0B, SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

Exact named invocation1, `running 1 test`1, named FAILED1, failed0-passed/1-failed summary1; native functional marker0, wrapper completed0, successful summary0 and S54_CAPTURE marker0. There are no advisory lines to quote. Both stream records report sourceEOF and stored completeness true, invalidUTF8/overflow/truncation/malformed protocol false/0; outer output.complete is false, with no successful protocol closure. Source prefix metadata is preserved: stdout325B/redacted8 lines and stderr41B/redacted1 line. Raw arbitrary redacted text was neither exposed nor used to infer a cause.

Known outcomes: harness68712 exited101 and was waited; launcher8784 exited124 and was waited. Internal outer runtime cleanup recorded `forced_cleanup=true`, `cleanup_verified=true`, owned_before `[8784,57020]`, owned_after `[]`. Launcher8784 is a known process; executable identity for residual57020 was not captured and is not guessed. Supervisory cleanup was separately nonforced/verified/empty. Native OS process inventory after the Run succeeded and found zero current exact-image processes. No foreign process was terminated.

`memory-observer-abort.json` is public evidence5301B, SHA256 `89ef39c6d157648ff59f4b4140dba99bc2702dfad5b2df43122164036d9cf009`; the final observer document is absent. Its presence is not a root-cause diagnosis. The owned synthetic `f-origin` directory and37 pairs of command stdout/stderr logs remain in place. Their private/profile or arbitrary message contents were not traversed, copied, exported or diagnosed. Only the explicit public artifact allowlist (14 files), public protocol metadata and necessary binding/count/hash facts are included in this handoff.

## Preservation and handoff

Postcheck verified all520 source pins, nine image pins, both original root inputs,565 immutable historical pins, HEAD/index and exact root navigation pins. F01/F02 run directories remain empty; F03/F04 retain their original eight failure artifacts. Legacy CLI/relay and prepared compiler captures/origins/dep-info/preimages remain unchanged. WholeP directories are absent. Finalization also verifies original preparation evidence and outside-report Git status; only this report and pins are new review artifacts. No compilation, image staging, full external validation/matrix replay, source change or commit/push occurred.

Adjacent `.pins.json` binds original public runtime artifacts, actual command receipts/invocations/captures, original preparation/root-activation evidence and preserved source/image/history pins. It references originals without duplicating large binaries/captures and excludes the synthetic private profile/command-log contents. The complete original outcome is retained; F05 cannot be reused even though the activation window has time remaining.

The concrete blocker for this stage is the failed single named F test after native rendezvous, with saved `harness_nonzero`, harness101 and forced internal cleanup. The exact functional check/error is unknown from permitted retained public evidence. Return for root review; no further test, diagnostic replay, activation refresh or next-stage work is performed here.
