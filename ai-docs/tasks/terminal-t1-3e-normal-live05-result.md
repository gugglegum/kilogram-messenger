# Ordinary two-host baseline: LAN admission and B shutdown failure

19 November 2026. Executor result for the accepted observer repair and normal run. Real messaging acceptance remains open. No product Rust fix or unreviewed runtime change was made after this run.

## Published inputs and execution

Accepted exact three paths were published as **433fd4f2bbb296968daf96d41140076fb2fd40e7**, parent **cfa6f85d57d65685c9cece3d25d7b69a237ccef4**, with successful ordinary push and live origin/master verification. Exact raw/Git blobs/pathset and empty index were verified; eight unrelated tracked edits and 9921 other notes were preserved. Compact receipt: `.tmp/terminal-t1-3e/normal-observer-publication-run-01/publication-final.json`, SHA256 **9efb3bc891266db10fd948f58cffb5ffedbb14bd984ebca167b9b7c13d753f50**.

Reviewed plan: `normal-observer-param-review-01/package/runtime-plan.json`, SHA256 **bd45789fa30e75ee1eeb52ac9c73eb5d2d2db12bbd7d19041433f97eeb06ecbb**. Actual command and source/admission pins are retained in `normal-observer-publication-run-01`. Evidence: `.tmp/terminal-t1-3e/normal-live-05`. Stable product images were the already accepted CLI736827d5...da1ac2 and bootstrapdac78993...a9e7a; no new Rust build. Maximum two host operations, BelowNormal, controller3600/outer3660, local attempt1000/1015 with controller cleanup reserve60, SFTP160/180/200 and metadata/localcopy80/90/100 were retained.

Controller exited1 at **18:18:27.756252UTC**, elapsed **458.125s**, without outer timeout. First normal-ssh-09 entered real agents/application execution and failed; normal-ssh-10 was correctly not started. Original failed01/02/03/04/05 artifacts and staging copies remain intact. This is one actual admission-failure observation, not a repeated-run success or a statistical failure-rate result.

## Actual outcomes

| Stage | Observed result |
| --- | --- |
| Input transfer/update | Both real six-input sets installed; hashes/readiness passed, including SFTP |
| Preflight | A/B identity, NIC/network and UDP-port conditions passed |
| Attempt binding | Both local native deadlines and lab-init identity/pin binding passed |
| Setup | Real filtered Setup completed on both; four invitation/reply/consent artifact transfers; matching two contexts and canonical roles |
| Chat preconnect | Both ready and disconnected; no runtime/IPC/socket before connect |
| Runtime publication | Both actual runtime/socket/token identities verified; both slots publication_ready=true |
| LAN admission | No four live leases within the admission window; no admitted phase receipt |
| Messages/history/events | Zero message sends. No delivery/retry/history/event acceptance |
| Offline/restart scenarios | Not reached |

There are **14 complete status samples on each host**. Every complete sample has both slots phase0, active=false, live_admitted=false, no lease expiry and publication_ready=true; policy errors are null. Both own locator revisions are1. A has peer revision0 and discovery state discovering in all14 samples. B has peer revision1 and state retrying in all14. Both own runtime endpoint/socket identities remain bound to the expected host/interface. These observations establish asymmetric locator progress; they do not identify packet loss, WFP filtering or a protocol cause.

The raw primary exception is a final observer `TimeoutExpired` with approximately **0.046s** remaining at the admission boundary. It must be retained as recorded, rather than rewritten as a successful or precisely diagnosed network outcome. Complete earlier samples show the unmet lease condition. The generic polling helper allowed a last metadata subprocess to start with this tiny remaining budget, so the top-level exception obscures the stage label. This is a separate harness diagnostic issue and a consideration for the next probe.

## Shutdown, native cleanup and restoration

A Setup and Chat owner receipts are clean: child0, acknowledged quit/EOF, no descriptor, no retained Job children and no forced cleanup. A attempt result is failed/LAB_CANCELLED because the controller cancelled after the primary admission failure; its native attempt Job is empty without force/error.

B Setup is clean. B Chat's actual quit reply is **ok=false/code=cleanup_unconfirmed**. Chat exits **3**, records connection last_outcome=failed-stop, and its host wrapper exits1/LAB_OWNER_FAILED. Its receipt nevertheless proves membership, mailbox EOF, no descriptor, before/after empty and forced=false; it does **not** prove an acknowledged clean quit. The controller records owner clean exit unconfirmed separately. B native attempt cleanup is also empty without force/error. Empty Jobs are not substituted for successful application shutdown.

Both original six-input sets were restored only after confirmed idle and known hashes. `normal-live05-state-01` confirms current native agent PID/FILETIME/image/session/SID/nonadmin, expected WindowsIdentity.Groups and unchanged full raw groups/attributes, idle/unblocked state, original hashes and baseline readiness. Existing A agent44024 and B11884 remain alive. No product owner remains; no privileged task, installation, rule/profile/account/VPN change was made.

## Diagnostic evidence and its limits

Bounded copied worker/owner/socket receipts and public connection metadata are in `normal-live05-diagnostics-01`; exact B quit command/reply are in `normal-live05-B-quit-01`. Both Auto LAN selections retain the expected local/peer IPv4, interface/network IDs and /24 prefix. No manual locator import was used.

A read-only rule scan found the owned Private inbound discovery/Iroh rules and the generic codex_sandbox_offline_block_outbound rule. Same-incarnation native AccessCheck against the actual A agent returns **-1 (excluded)** for that rule's LocalUser SDDL, which names SID ending1012. It must not be blamed merely because its program/address selectors say Any. Evidence: `normal-live05-rule-token-01/A.json`. No firewall rule was changed or bypassed.

The first NetSecurity collector retained valid A JSON but its strict collector rejected a long, partially serialized progress stream. A later COM view corroborates the scoped A observation. The installed B helper lacks the newer normalization function; a fallback COM collector timed out. Its own exact EncodedCommand/PID14028/FILETIME134395867275082692/image were rechecked and only that collector was terminated at18:34:21UTC; persistent B11884 was expressly excluded. All failures/raw bytes remain in their original folders. A complete independent B firewall scan is **not** claimed. Installed B baseline readiness still passes.

No packet capture or new system logging was enabled. The selected observer view did not retain full status2 stats/resources/context details, and no additional runtime shutdown reason beyond cleanup_unconfirmed is available. Root cause of admission asymmetry and B stop failure remains open. No unchanged retry was performed.

## Reproducible fixtures and proposed next slice

All failed data and receipts remain at their original A/B run paths. Additional verified copies are in `.tmp/terminal-t1-3e/normal-live05-fixtures-01`: A ZIP **06e09858fcb09a9af038ff425c25ecb25caa7fde554a65420989206e49e903ec**,109 files/241132 source bytes; B ZIP **1eacd9f31040cdd0ce70a91c9fb19a727c525338ad77f3f41ff3d46ac92bc516**,103 files/232923 source bytes. Every archived file's size/SHA256 was checked against the source manifest; B ZIP/manifest were transferred by the pinned SFTP client with native empty-Job proof. Only new separate archive copies were written. These are test fixtures with host/user-bound DPAPI state; original data was not rewritten or removed.

The baseline command, accepted manifest, thirteen exact source snapshots, image pins, captured generated observer inputs, native clocks, actual setup artifacts, all complete/failed samples, cleanup and fixture manifests are indexed in `normal-live05-result-review-01`. Previously consumed attempt IDs/evidence directories must remain immutable; any repeat needs a fresh reviewed execution plan.

Proposed architecture-reviewed next slice, not implemented here:

1. Make admission failure reporting explicitly phase/deadline based, retain the last complete samples, and avoid using a 46ms phase remainder as a new subprocess budget. Finishing a bounded observation must not turn a lease observed after90s into a pass.
2. Retain existing read-only full status2 stats/resources/context details and Chat monitor state in a bounded diagnostic run, separating locator publication/capture/admission progress. Preserve Auto LAN behavior and strict identity/lease checks; do not repair the result through manual locator import or policy changes.
3. Investigate B cleanup_unconfirmed independently, preserving application acknowledgement, exit status and native cleanup as separate criteria. Product changes require a separate reviewed scope; this result makes no product-fix claim.

No new normal scenario or modified runtime was launched after failure. Executor submits this baseline and proposed diagnostic slice for architectural review, then stops.
