# Sender-crash field01 stopped before Setup: watchdog pin mismatch

2026-11-20. Exact accepted field plan f22e9133 was activated on published5714080a with authorized=true as the only activation-template change. No source, plan pins or quotas were modified. Entry point was accepted sender-crash.py with --run and fresh sender-crash-live-01; BelowNormal, controller1800/outer1860 and existing native/phase bounds were retained.

The controller exited1 after48.781s, timeout=false. `.tmp/terminal-t1-3e/sender-crash-live-01/single-sender-crash-suite-result.json` records attempts=[], primary RuntimeError `SFTP client failed; inspect upload receipts`, and an A original6 restoration error `staging copy unconfirmed; preserve stage`.

## Functional verdict

Preparation failed before native hint-crash-01 began on either host. No Setup, context exchange, materialized pending messages or runtime fault occurred. Runtime-fault intent/result and sender-crash-result do not exist. The one-use fault capability was not consumed. Parent finals, crash exception/missing3, recovery proofs, delivery history and notification reconciliation are not exercised; this is neither product crash-recovery PASS nor evidence of a product durability failure. The failed attempt/evidence is retained and was not rerun under the same journal.

## Concrete transport/staging defect

The accepted crash harness changed migration-s5-l1-deadline.ps1 from original SHA256 `4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5` to reviewed candidate/source SHA256 `efbda74a7dddd7d44de353d0b7101f99a331ec6dc0a9efa6a5c292c14044a522`. The existing normal uploader still hardcodes the original hash at three execution sites:

- normal.py line314, upload_script: receiver bin/deadline.ps1;
- line350, sftp_stage_script: receiver metadata watchdog;
- line378, sftp_client_script: local project scripts watchdog.

A upload/update stage de0715d286bd4b32badacf14679318d1 completed: all six candidate inputs deployed and readiness/baseline inspection passed. B stage a0b76f60853f4082932464643fcc0403 prepared metadata only. Local SFTP wrapper failed with raw `Local SFTP watchdog pin` before creating its guard/client process; no SFTP client receipt exists. Receiver verification then rejected the metadata-only stage file set. This was a local strict hash gate, not a network timeout.

Automatic finally restoration on A created fresh stage36b8574aed704471a84df9c52f8df2f4 but its first transfer failed with raw `Stage watchdog pin`: A bin/deadline.ps1 was now the candidate hash, while upload_script still required the original. No restoration update was issued. Failed stages and raw CLIXML/stderr remain intact. Do not strip the actual error, accept arbitrary watchdog bytes, patch a running command or bypass the gate.

The fix needs reviewed phase/host-specific watchdog binding covering initial deployment and post-update restoration, with the local wrapper tied to the accepted source hash and receiver execution tied to an explicitly reviewed current bin version. Harness/source/activation pins must be reviewed before using that fix. No such changes were made here.

## Current state and cleanup limits

Read-only cleanup-readonly-02 evidence under `.tmp/terminal-t1-3e/sender-crash-execution-01` confirms exact agent identities and unchanged payload15, baseline/TaskSpec, inactive/unblocked state and no native runs/hint-crash-01 directory on either host.

A remains the known candidate6 with agent22656/session1f573d11b9874dcdaed9ab7ee4dc490f. Its same-session Local Job query proves singleton membership containing only that agent. Default READ-ONLY Validate of metadata-safe Restore plan b100c669 also passed with canonical15 metadata/policy; Action Restore was not supplied.

B remains original6, agent11884/sessiona6e8faadcf4d4979b422f85578ae7c64; no B update occurred. Its state is idle/unblocked and native attempt is absent. An initial diagnostic incorrectly expected the SSH collector's Local Job query to see the interactive-session job; it failed safely and was preserved. Corrected diagnostics show collector session0 versus agent session1 and explicitly mark that job query inconclusive, rather than treating an empty result as proof of no job. This follows the documented [per-session Local namespace](https://learn.microsoft.com/windows/win32/termserv/kernel-object-namespaces); no namespace/system change or force query was attempted.

Overall original6 cleanup is incomplete because A restore is blocked by the reviewed uploader's stale watchdog pin. B is already original6. Metadata-safe A Restore has not run because the required preceding original6 restoration has not completed. No force stop, guard/pin relaxation, rebaseline, ad-hoc replacement restore, new suites/build or second field activation occurred.

Return this failed preparation result, current preservation/cleanup facts and exact source mismatch to root for the required harness/cleanup review. Keep healthy known A and all evidence intact, then idle. No result commit before root acceptance.
