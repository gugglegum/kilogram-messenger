# C2-S focused wrapper source review — attempt01

2026-10-31. SOURCE ONLY on HEAD `21f800da49d9c70fcf72b532e028f89d648f322c`. Implements accepted contract `d45befe07bb943e3e88b50d99eb92ba008451d907fd52e3a7af02e0ac271027e` and root's contract-accepted constraints. Submission has two edited scripts, three new dormant scripts and this review. No compilation, dot-source, initializer/helper execution, fixture creation, native case, allocation, private access, ledger/navigation edit, commit or push.

## Focused revision after root corrections

This revision supersedes source-review10184B/SHA256 `9ea84acf09c3dcec1cd0376a0199b309a0e251d1375d5bf339257af4908e0d18` while attempt01 remains unactivated. Only the three requested items and resulting metadata/dependency pins changed. Helper `setResult` is now long: every uint SetSecurityInfo return fits without truncation, and -1 remains a distinct pre-call sentinel; nonzero native-error conversion remains checked. A successful FindFirstFileW acquisition now updates peakHandles immediately after ownedHandles, matching directory/editor acquisition and including the search in the <=9 bound. Both destination rows now use `actual file byte read`, while the four source/source-after rows retain derived-lock provenance. Pure JS SHA256 and independent .NET hashing of literals extracted from the PowerShell AST agree on both revised metadata bindings. Wrapper, real verifier/delegates, cleanup policy, cases, resource caps and supervisor completion gates are unchanged.

The earlier ParseText-only pass missed the uint-to-int semantic compile error. This revision corrects that assignment by type reasoning; no compilation or semantic-binding success is claimed. Future authorized compilation and actual checks remain pending.

## Exact source pins

New filenames in this table reside in this evidence directory.

| File | Bytes | SHA256 |
| --- | ---: | --- |
| scripts/migration-s53-cold-preservation.ps1 | 82836 | `4288f66bbc634f336fb962624fe15aaf6561f760f7d2c096579bb94e31711612` |
| scripts/migration-s53-cold-output.ps1 | 21296 | `9d4b2a4fed3ffd76a1393514dff69f77baa5e20567e25c171be1c4203ec3a4b9` |
| C-s53-cold-output-wrapper-01-attempt-01.fixture-support.ps1 | 21111 | `4838fa35fe41551191e1bccf33b9c54115e53587a46f1634b7804e807698b7c8` |
| C-s53-cold-output-wrapper-01-attempt-01.preflight.ps1 | 40694 | `bbd62d628ef9f8ac7d93ef1a9bb6ee45336739445e5e0e7fc5a0d89db9ffb8b1` |
| C-s53-cold-output-wrapper-01-attempt-01.watchdog.ps1 | 45206 | `c08085db048bd80d82abe227d72b4ac35ff95797a8f8250de0189fd9153ab535` |

Unchanged dependencies: deadline `scripts/migration-s5-l1-deadline.ps1`, 6038B/SHA256 `0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45`; fixed bundled pwsh.exe, 301368B/SHA256 `362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139`. Root-native/producer fixture helpers are neither loaded nor required. Worker and watchdog require their own exact hashes from a future root activation, avoiding circular embedded self-pins. Both independently enforce the same fixed dependency pins, exact 23-field activation schema, two selectors and three output names.

## Ownership change and refusal trace

Output script: closed factories 81–86; wrapper 249 onward. Before TakeDirectory, exact type/signature/delegate/hash checks and fresh observation filter leave caller state intact on refusal. The five-slot array and bounded diagnostic map are prepared before transfer. TakeDirectory, assignment into slot0 and Invoke are inside one try. Failed Take with no pending handle forwards the original ErrorRecord without disposing Core.

After successful Take, every failure terminates the exclusively owned fresh pre-capture transaction. Primary ErrorRecord is saved first. Guarded observations record managed pending state and Core facts before disposal. Wrapper invokes Core.Dispose **first**, then observes Core again. It never infers method entry from an exception type. Existing real owner TakeDirectory/Dispose, ACL verifier, creator and all Core binding/native-close logic are unchanged; only closed fixture admission/paths/PowerShell ValidateSet changed in Core.

Cleanup throw, failed observation, prior/post cleanup-unknown/native error or contradictory accounting sets cleanup/native-close unknown. An open pending handle is eligible for managed fallback only after a successful Core.Dispose plus known observations, zero post-Core accounting and no uncertainty. Otherwise fallback is skipped and uncertainty retained. No DangerousGetHandle or raw close/retry exists in the wrapper. Even eligible managed fallback marks native-close outcome unknown and cannot pass either focused case. Closed incoming wrappers are not disposed again. A closed wrapper alone never establishes native success; Core native close failure can invalidate the wrapper while retaining accounting/unknown. No double-close corruption claim.

Bounded primitive diagnostic keys are attached to the original exception; a completion marker is written last. Attachment failure leaves no passing marker. Saved ErrorRecord is forwarded with ThrowTerminatingError; no invariant PowerShell outer nesting is assumed. No success-stream objects are emitted. Worker saves the original structured binding cause/chain before assertions, including in a failed report, and does not retry Core.Dispose after a transferred failing wrapper. Caller still closes owner ancestors; normal duplicate Dispose remains idempotent.

## Two source-constrained cases

Fixed family: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-output-wrapper-01-attempt-01`, leaves `healthy/proof` and `output-acl/proof`. Core admission: lines98–106, 263–267, 1353. Path-free owner factories accept no arbitrary path. Helper has one fixed family and closed ordered case names; CreateDirectory uses CreateNew and refuses old/partial evidence. E is created solely by the unchanged real owner.

Worker `Inventory-Literal` (294 onward) contains canonical UTF8/no-BOM JSON plus one LF, two zero-length lock rows in each of the three producer-shaped arrays, with source-derived and destination-actual provenance kept distinct. Independent pure JS SHA256/JSON construction and .NET SHA256 agreed:

| Selector | Bytes | Inventory metadata SHA256 | Files / payload |
| --- | ---: | --- | --- |
| wrapper-healthy-r01 | 2013 | `3de9737b02c04660f89b36eda638267f846a2c66e8ba3286022274791620a136` | 2 / 0 |
| wrapper-output-acl-r01 | 2025 | `ea7a2bdab58553178221136f7fd467e24f29199f506529bb0aaf05a8a1ccd4e1` | 2 / 0 |

These are binding metadata only: no inventory file, origin/state/backup trees or lock files are created/read. No CaptureAndCompare, payload hash, receipt creation or WriteReceipts. The metadata's provenance strings establish no actual lock/capture/equality proof.

Both cases call the **actual changed Bind-S53ColdBaselineSmallOutput**, checking real delegate target, method and declaring type. Healthy witnesses owner7→6, Core1 after return→0 after caller Dispose, no failure/receipt/capture and E empty before/after. ACL helper first validates new protected three-ACE E, then keeps a nofollow deny-delete editor (READ_CONTROL|WRITE_DAC|FILE_READ_ATTRIBUTES), matches E identity and removes only Administrators allow ACE. Real current-user/SYSTEM ACE bytes, owner/group, protected present non-null valid DACL and setter result0/readback are checked. Unchanged owner Take has no ACL recheck; unchanged real verifier refuses inside Core. Expected original marks/private_acl and output_acl/output_binding/output/native0/HResult-2146233079/cause unexpected must survive forwarding. Catch diagnostics must precede wrapper Core.Dispose, incoming closed, Core owned0/known cleanup, no fallback. No independent verifier-entry counter claim. Both identities, ACL digests, exact retained ACE bytes, cause/chain, cleanup primitives and same-clock ordering are reported and checked.

## Explicit resources and future completion

Existing prefix directories E:/, AI-Projects, repository, .tmp are opened; helper creates only family+two case directories. Output owner creates two E directories: **five new directories, zero fixture files**. E has six retained ancestors; each owner prepares7, retains6 after transfer and closes6 (12 total). Helper retains seven root guards including both case roots, keeping identities/ACL snapshots. It opens healthy E twice and ACL E once; normal helper explicit closes are7+2+1=10, with editor remaining0, all7 root guards closed in Dispose. Four bounded empty-directory searches use at most one search handle each; actual opened/closed search counts must match (0..4). Helper peak<=9; combined explicit filesystem/search owners peak<=16. Core peak1; bind-only path charge0, record charge12,073,840B, scratch524,288B. Core's existing caps remain unchanged: handles40126, rows8000, entries20000, inventory16MiB, payload/output64MiB, path96MiB, record32MiB. No wider caps introduced.

Prospective work20/whole30, lane1/retry0, one supervisor and one worker; no allocation. Common parent QPC/start/frequency propagates to worker/Core/helper. Worker job work20/cleanup24; parent reap24/drain26/report28; independent guardian child25/self29; external whole30. BelowNormal fixed pwsh path, no network. Polling limits are sampled OS handles<=1024 and sampled lifetime peak working set<=1GiB, nominal100ms/max301 attempts; these are observed caps, not a hard OS memory limit or exact handle high-water proof.

Supervisor New-Drain/Pump/Note-ExitIndication/Note-EOF/Completion-Gate/Record-Wait/Wait-ForReap are byte-identical to accepted C2-R attempt02 except Stop-C2S naming. A real successful retained-Process WaitForExit (1..20ms clamped to remaining absolute time) sets reap once; HasExited/EOF alone cannot pass. ExitCode follows successful wait. Normal pass requires exit0 and both EOFs before common20, complete worker report/source pins and known cleanup. Pre-cleanup gate/first reason survive later cleanup; no failed run is promoted. Guardian remains armed through report/reader/process disposal and actual worker exit.

Reports: CreateNew only `started.json`, `cases.json`, `watchdog.json`; each<=65536B, total<=196608B. Stdio buffers1024chars, each retained<=4096chars; successful worker stdio empty. Diagnostics<=16 entries/6144chars total/2048 each; cause/chain<=16, type<=128; wait trace<=8. Source/public-input streams are read-shared without write/delete and must all close:9 in worker,9 in parent. Existing or partial outputs/fixtures, source drift, timeout, missing primary marks, unknown cleanup or unexpected fallback refuse and preserve evidence; no repair/retry.

## Static checks and stop

PowerShell Parser: all five submitted scripts, zero parse errors. Roslyn ParseText: three embedded C# literals, zero syntax diagnostics; assemblies were loaded only for parsing, no compilation/semantic binding. UTF8/no BOM/LF, no trailing whitespace/extra blank EOF. .NET disk hashes independently matched pure JS hashes; exact Core/owner reconstruction proved no edits outside declared fixture additions/wrapper scope. Scoped git diff whitespace check clean. HEAD unchanged, index empty; pre-existing unrelated worktree changes preserved.

These checks do not establish compilation, native ACL refusal, PowerShell forwarding shape or actual deadline/resource behavior. No focused case has run. Whole S5.3 stays open; private typed-open, G1–G4 and S5.4 remain inactive. Root source review and separate activation are required next. Callback root and STOP.
