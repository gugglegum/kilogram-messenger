# T1.3e startup-diag-03 — future announcement rejection confirmed

2026-11-20. One accepted diagnostic attempt completed; executor stops for architecture review. Source contract: [root acceptance](terminal-t1-3e-discovery-rejection-root-acceptance.md). No second attempt or product deferral fix.

## Publication and execution

Published **c835e3e0ac08767a4d14d7cebcb5261a30c706fc**, parent **7a8772b0edc0c1745f902c2c5e8bdc50bdb600fc**, exact11 accepted files, empty index and live origin verified. Proof `.tmp/terminal-t1-3e/discovery-rejection-publication-01/publication-final.json`, SHA256 `a945adfc7ed9a10f9bb057769be393398f227ef90ac03b333427d99c13340f11`. Foreign8 and unrelated notes preserved.

The immutable root template SHA256 `abd330953fce35572ff35ef4e26cca0a23ffc363f0f0e6e99a08831247a4d7c2` was materialized by changing **only commit**. Separate runtime acceptance SHA256 `288ba9e77e9d9def3cef5186bcaa7ccd47831f69c6e3f2edc6504da72c9248ab`, with comparison proof in the publication folder. The unchanged accepted wrapper verified acceptance, inventory134, source/image pins and published HEAD before execution. Plan SHA256 `b842999ea632c4ced2d35e6f0048af0de05294fe2bff18d3c96cea98177c83e0`.

Exactly one fresh **startup-diag-03**, evidence `.tmp/terminal-t1-3e/discovery-rejection-live-01`. Native work800/whole815/reserve60, controller1800/outer1860, BelowNormal. Controller finished in585.422s, exit1, **no outer timeout**. The inherited suite exception text says `two attempts/restoration/readiness unconfirmed`; the recorded primary failure is AdmissionFailure and both restorations/readiness checks succeeded. The generic text is not evidence of a second attempt or failed restoration. Neither controller exit1 nor its wrapper proves application cleanup; the independent receipts below do.

Both pinned original agents, native/host identity, network/ports, fresh Setup and all four public setup transfers passed. Two shared contexts and reciprocal public peer IDs agree. Fresh credentials invert the prior attempt's canonical roles: **A/PC role0**, **B/laptop role1**. Both Connect commands succeeded in17.187s/42.078s and returned owner mode/current runtime instances. Each side has23 resolved commands: preconnect status, one connect,20 statuses, one quit; one submit per command, no errors/outstanding commands. These are successful command/lifecycle results, not authenticated peer admission.

## Separate admission windows

Original first-connect origin235435.359, deadline235525.359;7 paired rounds, decision235526.812: **false, AdmissionDeadline**. The last complete snapshot was sent235520.062 and received235526.812 within its probe deadline235580.062, labelled `late_observation_not_admission`. It did not change the original verdict.

The complete negative snapshot qualified for the separately labelled diagnostic window: origin235526.812, deadline235616.812;13 rounds, decision235617.359: **false, AdmissionDeadline**. Total182.000s from first connect. No normal messaging acceptance, messages/offline/restart exercise or additional attempt.

All20 complete full-status snapshots per host remained phase0, activefalse/livefalse, no lease, publication_readytrue, own locator revision1, no policy error. A peer revisions stayed0; B peer revisions1. This agrees with the final runtime counters, not just a partial last sample.

## New field attribution

| Final runtime metric | A / PC / role0 | B / laptop / role1 |
|---|---:|---:|
|LocatorAttempt|41|40|
|LocatorOpened / Validated|0 / 0|40 / 40|
|LocatorCommitted / Duplicate|0 / 0|1 / 39|
|LocatorError|41|0|
|AnnouncementFuture|41|0|
|All other rejection categories, including Unknown|0|0|
|SendAttempt / SendOk|47 / 47|43 / 43|
|ReceivePacket / PeerFilterRejected / InboxQueued|88 / 47 / 41|83 / 43 / 40|
|MetadataStart / MetadataReady|84 / 84|75 / 75|

**The exact rejection invariant in this run is `issued > receiver.now`.** All41 owner locator attempts on the PC failed at the AnnouncementFuture site. Envelope/recipient/HPKE decryption/signature/body-version/time-field parsing checks precede that site and returned successfully for these packets. This is not a decryption failure. Remaining consent/locator checks occur later; these counters do not prove they would all pass after waiting.

Existing preflight clock receipts in this attempt estimate B ahead of A by `[2.5412241,4.8702241]` seconds, or `[2.5392241,4.8722241]` with2ms conservative rounding margin, assuming UTC reads inside their controller RPC intervals and no intervening clock step/significant drift. `.tmp/terminal-t1-3e/discovery-rejection-result-01/public-timing.json` records the public input hashes, formula, roles and limits. No additional clock sampling or setting occurred. This estimate corroborates the actual future-time rejection; it does not measure each datagram delay/offset or retrospectively recover the inner category of diagnostic02.

Direction follows the slower receiver despite swapped roles, matching the accepted synthetic matrix. In this role assignment A is the dialer but never obtains its discovery target; both slots end `waiting_target`, attempts/errors/timeouts/leases0. B is `not_dialer` for both slots. **ListenerIncoming, ListenerPrerequisiteRejected, ListenerActualTlsError, ListenerDeadline and the retained ListenerTlsError are all0 on both hosts.** No inbound connection reached either branch here; do not claim a new positive field test of listener prerequisite/TLS separation, or reclassify diagnostic02's33 aggregate listener failures from this run.

OS error maps are empty. Metadata/send/receive/quota/socket/endpoint/supervisor/transport/owner-cleanup errors and admission timeouts/errors are0. The source diagnostic goal identified the rejected invariant; the peer-admission scenario remains FAIL.

## Capture, shutdown and restoration

Both hosts produced14 typed records:9 successful startup stages plus runtime-stopping/network-stopped/final and connection/worker finals. Coverage complete, no missing/rejected records, no startup_failed. Final runtime counters include packets processed after the last status observation (A41/B40, versus last status39/38); use finals for totals.

Connection finals on both hosts: acktrue/conflictfalse/forcedfalse/exit0/eoftrue; both drains EOF, neither failed. Worker finals: running/saved/graceful/reapedtrue, forcedfalse, exit0/eoftrue. Runtime network/network_join/ipc/owner_join flags alltrue. Host receipts confirm child_exit0, mailbox_quit/eoftrue, work_expiredfalse, no error/force, no outside-job process before/after.

Both native attempt jobs ended empty without force or cleanup error; two owners clean per host. Agent terminal reason LAB_CANCELLED reflects the admission scenario failure, while owner/application exit results remain clean. Only after native-idle/known-content gates, both sets of six originals were restored. Independent final installed-state/.NET token and raw-token snapshots confirm unchanged agent PID/start/SID/session/groups, nonadministrator identity, idle/unblocked state and successful readiness on A and B. No manual launch/install/UAC/policy/firewall/VPN/account/system-clock action occurred.

Public owner/mailbox/connection receipts are in `discovery-rejection-owner-evidence-01`; final agent/native/raw-token readbacks in `discovery-rejection-final-state-01`. The original host test fixtures remain in their run directories; no failed private state was copied or decrypted. Result analysis/final facts/verification/evidence index and an exact1 proposed result-note allowlist are in `discovery-rejection-result-01`. The result note awaits root review/publication instruction.

## Next boundary

The field evidence now supports a separate product contract for handling a small future issue time. Do not implement or relax time checks here. The previously proposed one-datagram bounded deferral needs review of complete fresh validation, strict expiry/consent/locator/high-water/quota checks, monotonic waiting cap/cancellation, replacement fairness and resistance to authenticated far-future starvation. No source fix or repeated field attempt follows this report without that contract. Callback publication/result/evidence, then idle; no user action is required.
