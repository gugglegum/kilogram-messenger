# F-exhausted-boundary-03 — whole FAIL at private preservation

2026-10-26. Exactly one actual native-token attempt ran after the human directly
authorized case03 on fresh test copies with work780/whole800. Root review
`long-lived-dialog-a3-migration-s5-l1-boundary03-review-1.md` SHA256
`d7d3e065a515a2a24201f547a1fcd4c4ae4123dc5dc93659adc776bc25098bb3`
and root approval SHA256
`182ef50a362664d406f3f2e76f04207384ed3487b6bf12b3ab330dc5ef15da8e`
and all167 readiness references matched before launch. The prior Auto-review
denial remained a separate prelaunch record; it created no runtime or F charge.

## Actual outcome and cause

Whole **FAIL649.592232s**, runner exit1, matching stable harness exit101,
0passed/1failed after641.54s. Outer work780 did not expire; the controller760
deadline did not fire. The whole800 owner finalized its case and ledger. Owned
children after cleanup0; an independent process snapshot found no live Kilogram.
No product crash or full/partial functional PASS is asserted.

The error is in `checkpoint.private_preservation`, after the reviewed functional
checks, cold receipts and final raw origin verification. The normal preservation
helper's initial source inventory failed at `Get-FileHash` on
`boundary-phases.jsonl`: Windows reported that another process was using the file.
The controller's `Phases` still owned its open writer while invoking this helper.
The public log is hashable after controller exit. This observed error and source
lifetime are consistent with the retained writer preventing the helper's hash
open; no alternative execution or sharing probe was performed.

The failed phase began640.7879600 and returned the error641.5285082 after
0.7394024s. `Phases` then recorded controller error641.5376300. The helper source
places `$before=Inventory` before output-directory creation/copy. Its destination
`.tmp/migration-s5-preserved/F-exhausted-boundary-03-checkpoint` does not exist;
there is no successful private preservation inventory. The helper was not rerun
after the writer closed. No controller/helper/product fix was applied.

## Completed recorded path and missing results

The stopped branch is
`E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb`.
Leave it stopped; no database/state traversal, typed open, repair, resume or
fixture-state backup was performed after failure. Failed01/02 and the frozen
original were not retargeted or typed-opened.

Both migration children reached all six exact hooks and exited0:
A PID35068, B PID2812. Their persistent stdout reports `storage_stage=Ready` and
`zero_migration_native_calls=true`. Both controls contain `observation-released`,
with absent hit markers. The actual public owners were stopped before the cold
receipt checks and preservation.

| Completed phase | Controller elapsed at end / phase duration, seconds |
| --- | ---: |
| Migration A | 309.0528988 / 277.1241707 |
| Migration B | 605.4883808 / 274.8862581 |
| Fresh both-indexed reconnect barrier | 615.6342077 / 6.2057349 |
| Refusal/retry/conflict after reopen | 616.5974612 / 0.5385013 |
| Original frozen suffix | 623.0306892 / 6.4252386 |
| A528 history | 629.4225731 / 6.3830453 |
| B528 history | 636.3201346 / 6.8914666 |
| Owners final stop | 636.4158793 / 0.0282304 |
| Cold A receipt | 637.0108650 / 0.5907583 |
| Cold B receipt | 637.5537331 / 0.5390739 |
| Raw original verification after | 640.7840042 / 3.2263836 |
| Checkpoint-info write | 640.7868202 / 0.0003378 |

Reviewed sequential code plus these completions establish execution through the
identity/nonce/secrets/Session correspondence, zero constructor/Packet assertions,
exact retry/body conflict/new-request refusal, original527 history, actual B17
receive/ACK, fresh-pair barrier, exact frozen suffix, both528 histories and cold
receipt/frontier comparisons before preservation. They remain observations for
root review. There is no `parent-exhausted-result.json`: complete returned
statuses/history/session/barrier objects were still in RAM and were lost when
the test returned an error. Do not reconstruct them as original saved results
or relabel this whole failure PASS.

The immutable barrier snapshot is1778B and contains
`l1_rpc kind=9 stage=finished millis=581 result=reply9`. The accepted controller
only creates that snapshot after current fresh pair/lease/admission checks and
a later matching Resume, within local25s. The snapshot and successful barrier
phase survive; detailed PID/token/status return objects intended for the parent
result do not. The full fresh-A stderr log is also exported.

Actual `checkpoint-info.json`1614B survived. Both cold next values are[512,18],
debt/pending/queued0, receipts[511,17], sources530. Receipt hashes match:
`338a74bcbee1249cb1655fa21962833d6d680394769861ca2df4ab98475a4760`.
A Native generation517/hash
`6c50ea198a9d1c9fc6338c873a7bcda7d169c421d32d65fbe5369c3695c2d247`;
B generation88/hash
`e6e4abc55d1216a3514b40261005d6eb29e9fdecbc0312a78bddfc21fccccf1f`.
These are the controller's actual cold outputs, exported without another open.

Before and after raw original manifests are byte-identical1883572B SHA256
`cd7f2f4379be0c972083a03fe94346600afea091dbcd371fa3b910d51ba645be`,
checking the same frozen original6944 files/17855269 bytes within this charged
attempt. No additional origin traversal ran afterward.

## Exact evidence and retained inputs

| Artifact in this evidence directory | Bytes | SHA256 |
| --- | ---: | --- |
| `F-exhausted-boundary-03.json` | 12386 | `6da9c4ae43dad5234df1da2aba156e0a972a7d3895069d4cf87fcdbe7932cb48` |
| `F-exhausted-boundary-03.log` | 4493 | `739839e858e1a5131e0bbe09063851f4fc2ab193d96297fd2466679b78f4a38f` |
| `F-exhausted-boundary-03-observation.json` | 42598 | `7f2adda05a3e1a845b4b7580db54e402283fad608e0d2c24de98a7b82f5f05f0` |
| `F-exhausted-boundary-03-phases.jsonl` | 31316 | `940df52d347a96dcb48d1f62b7fbe2cdfc5440a62f96868f4872aa86de94c4af` |
| `F-exhausted-boundary-03-checkpoint-info.json` | 1614 | `2b359e4623b2e603977342df69f441e35e0276643ee420a751a3515e361771b3` |
| `F-exhausted-boundary-03-two-indexed-reconnect-barrier.log` | 1778 | `545f0849be3035bb86521b03bd3dce5fd972c472cce603f27ae5c722647c8b8f` |
| `F-exhausted-boundary-03-two-migration-a.stdout.log` | 969 | `7ca145d62530a84b3e0263a5f6f3fd20e6f9dd356bf10cf153f72396ced172cb` |
| `F-exhausted-boundary-03-one-migration-b.stdout.log` | 968 | `cdf69b5ad8543ec1b5e9e03353533a263eb9d928aefc2e9e08f9890c9b09c83d` |
| `contributing-cases.json` | 419189 | `acd66ac604bb83cfee19d99a26122ce5f345b2d4f3eea4d0791e644869251f6e` |

Observation binds all22 public exports to byte-identical source artifacts:
phase/checkpoint-info/schedule, before/after raw manifests, migration and owner
stdout/stderr, fresh-A complete trace and reconnect snapshot. No fixture state
was exported. After the attempt all54 case source/image references,33 product
inputs,eight images,49 historical references and the exact reviewed harness
matched. Product images remain69744fca...; controller c3e04459..., runnerd3324692...,
harness83429f6d... and preservation helper are unchanged. No source edit or
additional compile/test followed the failure.

## Accounting and stop

Current C147.8846192/F3263.7458077/P0 under ceilings500/3650/0. Remaining
F386.2541923. Exactly one new case row charges the full649.592232; there is no
refund, reset, extra amendment or P borrowing. Prior origin whole PASS,
boundary01/02 whole FAIL and all Ready02/capacity01 split outcomes remain intact.
The independent whole watchdog covers final publication; the stopped runner
stopwatch excludes its terminal case/ledger write tail as previously disclosed.

Stop and callback root with these actual artifacts. No second attempt,
preservation retry, product/controller correction, new C/F, product optimization,
whole-P, S5.3/cut3, implementation report/candidate allowlist completion or
commit/push is authorized after this FAIL by the current decision.
