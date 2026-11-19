# Admission diagnostic01: B connect incomplete, A observed and cleanly stopped

20 November 2026, Asia/Yekaterinburg. Result of the one root-authorized diagnostic attempt. **Admission is not accepted; diagnostic coverage is incomplete.** The attempt stopped in runtime_connect, before entering the90s admission polling loop. It is not a repeat of live05's full admission-window observation. No messages, offline/restart scenario or second attempt ran. No product behavioral fix was made.

## Accepted publication and actual execution

Exact17 source paths were committed/pushed as **4c62ff74e44f5fbaece815bd9851ee7d1d54f66b**, parent **32d54c6b114e9047992de31c943f6794bf3e917e**. Raw hashes, staged/committed Git blobs, exact pathset, empty index and live origin/master equality were verified. Receipt `.tmp/terminal-t1-3e/admission-diagnostic-publication-02/publication-final.json`, SHA256 **78c14d74ba2014bc05c23f7be1c926e9ec7751e652f37160559e6a986033080b**. Accepted allowlist SHA256 **3e8aac13a1999629f2f388229c047cda60a1dd305e42c7fef5ffeb4d668eda6a**. Eight foreign tracked edits and existing notes were preserved. An earlier preparation guard noticed that the excluded chat-coordination.md had changed since the old preparation snapshot; publication01 stopped before staging. Its failure is preserved. Publication02 reverified all17 accepted pins and preserved the coordination file's current bytes, as root explicitly required; it did not include or reset that file.

Root's immutable acceptance template was preserved. A separate `admission-diagnostic-publication-02/runtime-acceptance.json` replaced only commit with4c62ff74; SHA256 **0708988939fab92acdb3ce7d1bb8264c7741f36c398cbe3cd3d75af7a362e9bb**. The accepted, unchanged `admission-diagnostic-review-02/run-bounded.py --run --acceptance ... --acceptance-sha256 ...` wrapper was used. Plan SHA256 **b2e4211146a83461320b2fbd48a6559b983d64b81b39b06866d74aa6d597de83**, inventory **97fe43cb0d930f3845e00df00275a6f8cc89a4ac24abfa258accf24204c0410a**, build manifest **8ecbb92366290871c998dfeb8a052b8004e08e84014c72eae80a6494dd6392e8**. CLI28cbc7f2...baf455/bootstrapcdd630b0...0506c were installed under the accepted stable paths, then restored to originals.

Evidence `.tmp/terminal-t1-3e/admission-diagnostic-live-01`, actual attempt **admission-diag-01**. Controller exited1 without outer timeout in **407.282s**, completed at **01:04:24+05:00** (raw receipt2026-11-19T20:04:24.055789UTC). Native attempt1000/1015, reserve60, controller3600/outer3660, BelowNormal/max2, observation90/24/min2 and transfer limits remained unchanged. One begin per host is preserved in request ledgers. The suite's `attempts=[]` lists successfully completed attempts, not the number actually begun.

Both preflights, agent/native identity bindings, input updates and real Setup passed. Four public invitation/reply/proposal/final artifacts transferred; both contexts and AutoLAN selections matched. Setup owners exited0 with acknowledged quit/filtered EOF and no forced cleanup. Both post-Setup diagnostic markers and Chat preconnect proofs were captured. Unlike live05, fresh canonical roles were **A=proposer0, B=cosigner1**; old role assignments must not be reused for this attempt.

## Connect failure and admission verdict

`admission-result.json`: stage runtime_connect, admitted=false, raw cause **RuntimeError: pilot deadline**, start228562.468,90s cutoff228652.468. The connect phase fails at approximately228587.484, i.e.25.016s after start. A single separately labelled `after_connect_failure_diagnostic_only` probe ran from228587.484 to228590.500, within its24s budget. There are **zero normal admission rounds**, so this run does not establish that four leases failed to appear throughout90s.

A connect completed and one full native-bound status2 plus serial Chat monitor were retained. B connect had no successful reply available before the controller deadline. Later copied B public mailbox records show command0002 connect/start and reply0003 **ok=false/code=cancelled**, not a successful connection. B's subsequent monitor request failed. The cancellation reply was written during subsequent cleanup; it must not be rewritten as a precise internal startup error or assumed to have been available before the primary controller deadline.

The reviewed diagnostic controller explicitly bounds the whole connect call to25s. Source `kilogram-first-dialog/connection.rs::Session::start` performs prepare/import/profile work before spawning, then gives its internal startup loop30s. Thus the outer25s envelope can end before the application's own startup budget, and this is a material diagnostic confounder, not proof of a product startup defect. A took about14.6s from mailbox connect submission to its successful reply. B's remote submission followed A's by about4s; its cancelled reply followed its submission by33.158s. Those wall timestamps supplement, rather than replace, the controller's monotonic deadline evidence.

## A stage observations

One4359-byte full status2: CLI runtime PID38360/FILETIME134395921724196470/native image/token/instance verified, both slots role0/phase0/active=false/live_admitted=false, publication_ready=true, own revision1/peer revision0, policy_error=null; discovery discovering. Both Chat monitor contexts are waiting_history. The public Chat status remains an owner observation, not an admission pass.

Final runtime checkpoint after14.675s lifetime:

| Stage | A measured total |
| --- | --- |
| Metadata | Start8, Ready8; Error/Late/Revoked/cleanup errors0 |
| Discovery socket / endpoint | SocketOpen1, EndpointBound1; open/bind/attach errors0 |
| Announce / queue / send | AnnounceAttempt5/Ok5, PublicationQueued5, SendAttempt5/Ok5; error/drop0 |
| Receive / outer filter | ReceivePacket5, PeerFilterRejected5; ReceiveError/Stale/QuotaDrop/InboxQueued0 |
| Locator codec / commit | Attempt/Opened/Validated/Committed/Error all0 |
| Dial / admission | Both slots waiting_target, attempts/errors/timeouts/leases/fresh totals0; listener/admission totals0 |
| Shutdown network stages | Supervisor/metadata-monitor/transport/socket/endpoint/owner-detach error totals0; last_os_error_by_stage empty |

This establishes local successful send API calls and received datagrams rejected by the existing outer source/interface/destination filter before locator processing. It does not show which predicate rejected them or identify their sender. Equal send/receive counts are compatible with local multicast loopback but do not prove that hypothesis. No authenticated peer locator pair, outbound dial or lease was measured on A during this short interval. These facts do not establish packet loss or a firewall cause.

## B observability and shutdown

No B runtime socket receipt, full status2, Chat monitor or runtime/connection/worker diagnostic sidecar was obtained. The typed collector returned empty records and no rejected files; coverage correctly lists all five kinds missing. Public connection metadata has a prepared profile/hash and last_outcome=prepared; no IPC descriptor remained. There is no bound B runtime PID/instance, stage counter, internal ack/conflict/runtime-exit/EOF result or supervisor/metadata/socket/endpoint checkpoint to report. Missing records are unknowns, not zero counters, proof of no spawn, or a clean stop.

Post-run read-only copied owner receipt establishes B Chat owner17292/child8644/membership=true, **child_exit=null, mailbox_quit=false, mailbox_eof=false, forced=true, before=[8644,2964], after=[], work_expired=false**, descriptor=false, generic Mailbox launch/stop/exchange failed. Owner worker receipt is failed/exit1; native agent result is LAB_OWNER_FAILED with owners_clean1 (Setup only). PID2964 is only an owned child ID in that receipt; its image/command were not retained and are not inferred. No actual quit command/acknowledgement was observed for B Chat. The empty final native attempt Job is a later, separate cleanup result and does not undo the owner-level forced cleanup.

A has all five diagnostic kinds with complete coverage. Connection final: ack_attempted=true/ack=true, descriptor_read_ok=true/conflict=false, forced=false, child exit0, both successful drain EOFs. Worker: running_ok/saved_ok/stop_graceful/reaped=true, stop_forced=false. Runtime stopping/network-stopped/final all present; network_ok/network_join_ok/ipc_ok/owner_join_ok=true. Host receipt also confirms Chat exit0/acknowledged quit/EOF/empty before-after/no force/no descriptor. A's agent result LAB_CANCELLED reflects cancellation of the failed overall test, not an unclean A application exit.

## Native cleanup, restoration and evidence limits

Both final native attempt receipts prove empty_job=true, forced=false, error=null. This coexists with B's earlier owner forced=true and must be reported together. The controller restored all six known originals on both hosts after idle/unblocked confirmation. Independent `admission-diagnostic-final-state-01` read-only native/.NET and raw-token checks again verify A44024 and B11884, exact FILETIME/image/session/SID/nonadmin/groups, idle, expected original hashes and baseline readiness. No product owner remains in either attempt Job. Existing agents were preserved; no installation/UAC/task/rule/profile/VPN/global capture was introduced.

`admission-diagnostic-owner-evidence-01` preserves bounded copied A/B worker/owner/socket/public-mailbox metadata and connection observations, with content pins. `admission-diagnostic-result-01/summary.json` retains the complete structured result and status/checkpoints; compact.json is a derived convenience view. Original new A/B fixtures remain at runs/admission-diag-01; old live05 fixtures were neither restarted nor rewritten. Native cleanup/restoration is established; full two-host diagnostic capture and messaging are not accepted. Previous compilation/targeted tests remain valid; strict Clippy remains the recorded pre-existing failure, not a pass.

Next source-review question: distinguish B connect preparation, helper completion, runtime spawn, descriptor/status validation and host/mailbox stop using minimal typed, bounded checkpoints, and reconcile the outer25s connect envelope with the application's prepare-plus30s startup lifecycle. Do not increase timeouts or infer a protocol/firewall fix from this incomplete run. No further runtime attempt or behavioral product change is performed. Result note is submitted for root review before its own publication.
