# Startup diagnostic02: complete capture, admission failed, both owners stopped cleanly

20 November 2026, Asia/Yekaterinburg. One accepted startup-diag-02 ran. **Original90s admission=false; separate diagnostic90s=false; diagnostic capture_complete=true.** Both Setup/Chat owners and native attempt Jobs stopped cleanly, and known originals were restored. This is useful failed integration evidence, not messaging acceptance or an established inner product cause. No messages/offline/restart/manual locator/second attempt or speculative product repair.

## Accepted publication and run

Published exact11 as **3bcaf4cd2f762c1eff161620bd4410faedccc6ef**, parent **c8cb05cf568c23be755dfd1444f85e18f55b8659**. Raw/staged/committed blobs, exact pathset, empty index and live origin/master equality verified;8 foreign tracked edits and9930 notes preserved. Receipt `.tmp/terminal-t1-3e/startup-diagnostic-publication-01/publication-final.json` SHA256 **c58b7555cccb65b9c24ae688f530eb29258c1355a4d6b2e581996e09086e09f8**. Allowlist **b591bab271cc3dcf55a41417194f5a2e71988bb549bd173a7ed5d496fa0e2e01**.

Root template SHA256 **e71d0f7c26bfdd04c7f7c75215869e2dbed133112d46d681d52ef39b253a5738** stayed unchanged. Separate runtime-acceptance.json replaced only commit; SHA256 **a03b9742d9ab16a7c21c621e93600d439915fa91939443fba663578fb15d5f22**. Used the unchanged accepted startup-diagnostic-review-02/run-bounded.py --run guard. Plan **87ce084414d2bdc379929e84e8b5be7372d465190572375514839dbf26ed31bf**, inventory **08a9c5a373fd44f1531c1ac75bccae02cac160ddfbe633a06975006b954c5526**, unchanged build manifest **2057652791ab1b19d330636b7aed5752638ebb513059036ab2e685aa44b41a35**. Accepted CLIe6cf411a...d833c6/bootstrap81a26f84...ca1b0d ran under the stable bin paths. No rebuild was done in this execution stage.

Evidence `.tmp/terminal-t1-3e/startup-diagnostic-live-02`, actual **startup-diag-02**. Controller exit1, timed_out=false, elapsed **583.515s**, completed **02:09:08+05:00** (raw2026-11-19T21:09:08.702874UTC). Parent attempt result elapsed443.485s. Suite attempts=[] is its successfully completed list; one begin per host and actual Setup/runtime/receipt ledgers prove one executed attempt. Native1000/1015/reserve60, connect85/probe60/mincall2, controller3600/outer3660, BelowNormal/max2 and transfer limits stayed as reviewed.

Pre-update live actor/original-byte/idle/readiness gates, updates, preflights, native clocks, real Setup/four artifact transfers/two contexts/AutoLAN, Setup shutdown, diagnostic enable and Chat preconnect all passed. Canonical roles in this fresh run are **A=cosigner1, B=proposer0**. Setup and Chat each have their own clean host receipts on both nodes.

## Startup and command correlation

Each Chat ledger contains24 requests: disconnected status, one connect t12e-2,21 status observations and one quit t12e-24. Every request has its reply, exactly one adapter submit attempt, no recorded transport/observation error, and **zero outstanding** at completion. Connect and quit full public results are retained alongside creation/submit/reply timestamps. No fresh connect ID/retry or quit preemption was needed in this live run; the latter remains focused-test coverage rather than a field claim.

Connect observed from request creation to matching reply: **A20.234s, B35.578s**, both successful owner/context/instance bindings. B's successful response in this run is later than the former25s harness cutoff and within the reviewed85s observation envelope. This directly demonstrates why25s was insufficient here; it does not reconstruct the previous B run's inner delay or prove the sole cause of its forced cleanup.

Both have nine expected successful startup kinds, no startup_failed, all cancelled=false. Before spawn, runtime PID/instance are null; after spawn only the actual Child PID exists; instance first appears after validated status. Startup elapsed_ms is measured from Session startup begin, not from command creation or the earlier setup-open marker:

| Checkpoint | A ms | B ms |
| --- | ---: | ---: |
| Import begin |11|7|
| Import end, ok=true |9646|16951|
| Profile helper begin |10073|17906|
| Prepared, ok=true |10241|18241|
| Spawned |13851|24141|
| Descriptor observed |14234|24872|
| Status validated |14698|25845|

A Chat36080/runtime36844 and B Chat4600/runtime18264 have native-bound status observations and typed nonce/PID/instance-linked receipts. Preparation and startup completed; there is no startup timeout or cancellation in this attempt. No helper stderr/private output was exported.

## Distinct observation windows and completeness

Original origin232278.750/cutoff232368.750,8 paired rounds, decision232369.937: AdmissionDeadline, admitted=false. Its last complete snapshot started232363.218 and completed232369.937, before its independent probe deadline232423.218; label late_observation_not_admission. Both expected hosts have complete runtime+monitor captures and coherent timestamps, no top/per-host/global capture error. The corrected eligibility permits the additional window; skip_reason=null. A late snapshot did not alter the original cutoff/verdict.

Separate post_original_window_network_diagnostic_only origin232369.937/cutoff232459.937,13 paired rounds, decision232460.656: AdmissionDeadline, admitted=false. Total observation time from first connect origin181.906s. Original and extra results remain separate; normal_scenario_accepted=false.

There are **21 complete full status2 files per host**, all with both slots phase0/active=false/live_admitted=false/no lease, own revision1/publication_ready=true/policy_error=null. A peer revisions remain0/discovering; B peer revisions remain1/retrying. Chat monitors are retained for both hosts. All five final checkpoint kinds and nine successful startup kinds per host were collected; coverage complete=true/missing=[]/rejected=[] on both; global capture_errors=[]. The scenario failure is admission, not incomplete diagnostics or cleanup.

## Final measured stage totals

Values below come from runtime-final after networking stopped, not a partial earlier sample. Runtime lifetimes are A164.515s/B156.439s.

| Boundary | A, role1 | B, role0 |
| --- | --- | --- |
| Metadata / endpoint | Start83/Ready83, SocketOpen1/EndpointBound1 | Start79/Ready79, SocketOpen1/EndpointBound1 |
| Announce / queue / send |51/51/51, SendAttempt51/Ok51 |42/42/42, SendAttempt42/Ok42 |
| Receive / outer filtering |ReceivePacket92, PeerFilterRejected51, InboxQueued41 |ReceivePacket89, PeerFilterRejected42, InboxQueued47 |
| Locator processing |Attempt41, Error41, Opened/Validated/Committed/Duplicate0 |Attempt46, Opened46, Validated46, Committed1, Duplicate45, Error0 |
| Dial / handshake |Both not_dialer; attempts/errors/timeouts/leases0 |Slot0 attempts17/errors17, slot1 attempts16/errors16; both last entered connect, timeouts/leases0; OutboundAdmissionError33/Timeout0 |
| Listener |Incoming33, PeerRejected0, ListenerTlsError33, InboundAdmissionError/Lease0 |All listener/admission counters0 |
| Other errors |Receive/send/announce/socket/endpoint/metadata/supervisor/transport/owner-detach errors0; OS map empty |Same error totals0; OS map empty |

Both nodes admit received datagrams through the existing outer route filter into the discovery inbox. On A every measured accept attempt returns an error before the LocatorOpened checkpoint; the measured failure lies after receive/capture and before successful open completion. That interval also includes generation/quota/local scope checks before open, as well as envelope parsing/authentication/decryption; current counters do **not** distinguish them. B successfully opens/validates/stores A's pair and repeatedly fails outbound connect. Zero leases and phase0 are consistently observed.

ListenerTlsError includes the pre-TLS owner.listener_pair lookup, not only incoming.await/TLS. Source runtime2/runtime.rs and discovery.rs::listener_pair require a current pair before accepting the incoming connection. A has no measured committed pair/peer revision, which is consistent with this guard refusing B's attempts. The counter does not record the exact inner rejection branch, so this remains a source-supported explanation, not an observed TLS/crypto reason. No firewall, cipher/key/role or protocol fix is inferred. Equal local send/reject counts also do not prove loopback identity.

## Clean shutdown, native proof and restoration

For **both** connection-final receipts: client_present/was_running/ack_attempted/ack/descriptor_read_ok=true; conflict=false; force=false; runtime exit0/success=true; two drain EOFs=true and failed=false. Worker running_ok/saved_ok/stop_graceful/reaped=true, stop_forced=false. Runtime-stopping/network-stopped/final all present; network_ok/network_join_ok/ipc_ok/owner_join_ok=true, cleanup error counters0.

Both host Chat receipts independently give child_exit0, mailbox_quit=true, mailbox_eof=true, before/after=[], forced=false, descriptor=false, error=null, work_expired=false. Quit t12e-24 replies are ok=true/value connected=false. Public connection last_outcome=graceful-stop on both. This run's B stop is clean; no claim is made that the older forced-stop cause has been reconstructed.

Final native attempt Jobs empty=true/forced=false/error=null, owners_clean2 on each. Agent results LAB_CANCELLED are due to the primary failed admission test, not application shutdown failure. Controller cleanup=[] and diagnostic capture complete=true. After verified idle/unblocked, all original six hashes were restored on both hosts. Independent post-run native/.NET and raw-token checks again verify unchanged A44024/B11884 FILETIME/image/session/SID/nonadmin/groups, originals/idle/readiness. No attempt-owned product process remains. Existing agents, Private rules/profile and old failed fixtures were preserved; no UAC/install/policy/VPN/global capture change.

Bounded copied public owner/worker/mailbox/connection evidence is in `startup-diagnostic-owner-evidence-02`; native snapshots in `startup-diagnostic-final-state-02`; structured analysis in `startup-diagnostic-result-02/summary.json` and compact.json. Original host data remains in runs/startup-diag-02; no fixture was restarted in place. Strict-Clippy and old helper-fixture test limitations remain as previously reviewed, not broad test passes.

Next architectural question is a narrowly typed rejection distinction within A's pre-open discovery acceptance interval, optionally supported by an explicitly reviewed offline reproduction from preserved test fixtures. This result does not implement or execute that next slice. Submit this note/evidence to root, then idle; note publication awaits review.
