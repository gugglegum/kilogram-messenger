# Sender restart live01: PASS

2026-11-20. Accepted exact4 implementation published as `f2276322da4a6b1648da77ecd12977fb3c92422b`, parent `050e649b6f803f43285e162dbc3a12b7514c62c9`; ordinary push succeeded and live `origin/master` matched. Allowlist SHA256 `c8a6eb0fd66145eb1454e739df7cb98b34b92aef7ffbcc2da8a377e3f76be8f1`; publication receipt `.tmp/terminal-t1-3e/sender-restart-publication-01/publication-final.json`, SHA256 `c72cd36bdee235190ff4ff67c5833754228be7afa18f2cdd9aea42c50e1ed62d`. Index remained empty and unrelated work was preserved.

Root's conditional activation was materialized by changing only the reviewed template's two approval booleans and verified commit. Acceptance SHA256 `8719f898d01c0d53a32e074f3cff19d835fb15530aa4f105b5f6e4f69001492f`; issuance and authority are retained in `sender-restart-prep-02/conditional-activation-01.json`. Fresh read-only preflight checked exact existing A/B processes, ordinary native/raw token views, installed payload/readiness, idle state and original six files before deployment.

## Executed scope and identities

Single attempt `hint-sender-01`; raw evidence `.tmp/terminal-t1-3e/sender-restart-live-01`, reviewed wrapper/plan/inventory unchanged (`23bf8246…` / `0f46ba4b…` / `81123a19…`). Controller exit0/no timeout, 640.391s; application522.266s. Native1000/1015s, cleanup reserve60s, controller1800/1860s, BelowNormal/max2 and original operation limits were retained. Existing candidate `161405ec3a5fd5c13bf6e1ef9765563dd722f716e07911250526545565b8629d` images were reused: CLI `af28682fbbe1cd8e4b3c5d4af0e6e7d41fe2f141cb502d707682d6b285418a60`, bootstrap `d11d8041eb5042c2b2805f9e0ca38be9f09a5e816c52df313b6f74df4e25d358`. No Rust rebuild/test or product source change.

Fresh Setup established both contexts; initial histories were0/0/0/0. Initial Auto admission was observed at60.281s, decided at63.734s within its original90s window. A held role0, B role1. B shut down gracefully and its runtime socket disappeared. A materialized one request per context; two observations before stopping A retained the same keys/events and exact one-row local histories.

| Slot | Original request ID | Original event ID |
| --- | --- | --- |
| 0 | `5194ea59c19b49f2a4b8a27fb3e1ad4e` | `ec8c066741e1fa56f0d2b3d6bb902aa49db239ddf323a7a786550e1adbfa4da3` |
| 1 | `f6e528ae19b3437e814476b9ac6b1e59` | `ae9e7b89f9d585cd5a906a164d26ab8fa2f12141d74abfca6b1da99c176940ba` |

A stopped gracefully and reopened the same profile as runtime `c579c8f56c1821a0e72131c7a5b7bb25109fbff4361c445a22c2df158eac08f8`, monitor epoch2. **Both post-A-restart observations passed while B was still stopped**: watching, original request/context/event identities, materialized status and unchanged full local rows. `hint-sender-01/sender-recovered-1.json` and `-2.json` retain each complete proof. B was started only after the second proof. Shared remaining work at the pre-A-restart and pre-B-return checkpoints was599.952/555.030s, with cleanup reserve unchanged.

B reopened its same profile as runtime `8bf5fc6c905eaa107803941d66f46f9a5bfd695a9d73d9b07035cd3b4dfdcccb`. Both new runtime identities and advanced own locator revisions were checked. Reconnect admission was observed at59.734s, decided at63.172s within the original90s reconnect window. Both queued events automatically became delivered with the exact IDs above; queued histories had one row/context/host. B then replied once per context. Final histories contain exactly2/2/2/2 rows and four logical events. No old request was resubmitted or retried.

Final actual Chat command counts: old A19, old B9, new A23, new B15, below192 each. Send counts2/0/0/2; retry0 everywhere, including after final drains. Replies bind the **new A** instance/epoch2. Complete-set notification reconciliation includes all four Chat incarnations (and the two empty Setup event scopes): required3, observed4, not covered0; required_passed and coverage_complete true. One queued event was already present at B's first watching history and therefore optional under the accepted anchor rule; its exact notification was nevertheless observed. Both online replies and the other queued event were mandatory and confirmed.

## Cleanup, diagnostics and final state

| Host / Chat scope | Chat PID | Runtime PID | Typed records | Validated prior records |
| --- | --- | --- | --- | --- |
| A / old | 47560 | 32820 | 14 | 0 |
| A / reopened | 47924 | 604 | 13 | 14 |
| B / old | 13224 | 12972 | 14 | 0 |
| B / reopened | 18088 | 1004 | 13 | 14 |

All four typed scopes are complete, without rejected records; saved decoded bytes/hashes match their collection envelopes. Each includes connection_final, worker_final, runtime_stopping, runtime_network_stopped and runtime_final. Warm scopes omit only startup_profile_begin, consistent with reopening an existing profile. Per-host cumulative recorded usage is27 records plus marker; no evidence was removed or overwritten. All four Chat receipts have childexit0, mailbox quit/EOF, verified membership and empty before/after Job state, without forced stop, descriptor remainder, expiry or error. Both Setup processes also exited0. Native attempts report closed/empty, owners_clean3 per host, without forced cleanup or primary errors.

Typed counters retain early admission observations: outbound admission errors4 on each A incarnation, listener prerequisite rejections4 on each B incarnation, before eventual successful admission. Listener actual TLS errors and transport cleanup errors are0. The inner rejection predicate remains unattributed; this successful run does not resolve that separate diagnostic question or change its gates.

The suite has failure=null and restored original six files on both hosts. A final read-only check after restoration confirmed original hashes/readiness, idle/unblocked state, no test processes under the existing read-only checks, and exact ordinary native/raw agent tokens:

- A PID44024/start134395772085441302; native10:45:10.168UTC, raw10:45:23.654UTC.
- B PID11884/start134395656927981748; native10:45:21.926UTC, raw10:45:32.145UTC.

These are final observations at the stated times, not an ongoing availability guarantee. Final state sources are `.tmp/terminal-t1-3e/sender-restart-result-01/final-state-01`. Machine-checked facts and exact full context/request/event/instance bindings are `public-facts.json`, SHA256 `907cf4db3e7362ba1ad3c9e0fb6f70d29615ff862d8cfea7f9f95c252bb952c7`. Its local verifier rechecked persisted proofs, histories, notification oracle, command logs, typed bytes, Jobs, restoration and final identities. Two result-analysis schema assumptions were corrected (the notification artifact includes Setup scopes; old reader envelopes omit the optional prior-count field); their initial scripts are preserved. They caused no field failure or rerun.

This proves the accepted graceful sender-and-recipient restart scenario with the recipient absent during sender recovery. It does not add crash, network-loss, Internet or arbitrary long-duration recovery coverage. Earlier failed/successful recipient-offline evidence and all preparation checks remain intact. This result note is submitted for independent review; no subsequent stage or result publication is started.
