# T1.3e discovery rejection slice — source review

2026-11-20. Executor stops at source/package review. Contract: [startup-diag02-root-review-next](terminal-t1-3e-startup-diag02-root-review-next.md). No new field execution or product acceptance-policy change.

The accepted diagnostic02 result note was published as **7a8772b0edc0c1745f902c2c5e8bdc50bdb600fc**, parent **3bcaf4cd2f762c1eff161620bd4410faedccc6ef**. Exact one-path/raw/blob/index/origin proof: `.tmp/terminal-t1-3e/startup-diag02-note-publication-01/publication-final.json`, SHA256 `37c21048fd0d11708c4fe3f8ddc8cb9687ee0846371cc808388f23a275baa85e`. This note publication is complete; the new source below is uncommitted and awaits root review.

## Evidence and reproduction

The prior field FAIL remains a failure to observe four authenticated live leases. `Opened0` does not identify HPKE decryption failure: `open` also verifies signature/body/context/consent/locator/time. Both applications and owned runtime/native children exited cleanly in that attempt; controller exit1 reflects the failed admission scenario.

Existing public `setup-complete.json` shows reciprocal account/device IDs, two distinct contexts in the same sorted slot order, A=cosigner/role1 and B=proposer/role0. No public role or slot-order inconsistency was found. No failed private fixture was copied, decrypted or restarted.

The existing preflight clock receipts support a bounded historical clock-offset hypothesis. Their UTC delta is6.3264083s. A was sampled within controller monotonic `[232052.765,232053.546]`, B within `[232055.125,232056.828]`. Subtracting the possible sample-time difference gives B-minus-A offset `[2.2634083,4.7474083]` seconds, or `[2.2614083,4.7494083]` with a conservative2ms rounding margin. This assumes each UTC read occurred inside its RPC interval and no intervening clock step/significant drift. Different machine uptime ticks were not compared. Derivation, input hashes and assumptions: `.tmp/terminal-t1-3e/discovery-rejection-review-01/public-timing.json`; reproducible helper `public-timing.py`. No fresh SSH/clock-setting operation was needed.

Fresh in-memory synthetic credentials reproduce the directionality under the unchanged strict `issued <= receiver.now < expires` rule:

| role0/role1 clocks | role0→role1 | role1→role0 |
|---|---|---|
|1000/1000|accepted|accepted|
|1000/1003|accepted|AnnouncementFuture|
|1003/1000|AnnouncementFuture|accepted|

Each row is checked with both receiver slot orders:12 cases. Thus the failure follows the sender clock being ahead, rather than the proposer/cosigner role or slot order. Thirteen representative failures cover size, wrong local recipient key, altered ciphertext, context, consent, locator identity/context, route, future/expired time, signature, duration, locator parsing and locator interval. Returned error variant/text match the no-observer path. Exact issue-time and just-before-expiry positives are retained.

This establishes a reproducible availability limitation when freshly issued datagrams arrive before the receiver's wall clock reaches their issue time. It does **not** retrospectively prove the inner reason for A's41 field rejections: diagnostic02 captured neither per-datagram issue/receiver time nor rejection site. Packet transport/queue delay and later clock behavior remain unknown. A new fixed rejection category is needed to close that field attribution.

## Diagnostic source only

Protocol `open_observed` exposes a fixed enum and calls the observer exactly once on error, never on success. Existing `open` uses the same single validation implementation with a no-op observer. Site markers surround the existing checks and preserve their errors/order; the time predicate retains its short-circuit order while distinguishing future, expired and duration failures. `seal` shares the same validator. There is one HPKE decrypt per invocation, no second validation pipeline, error-string classifier, raw payload/plaintext/key output or changed acceptance rule.

The owner adds a fixed26-key `discovery_rejections` numeric histogram: generation/freshness, quota, local scope, observation, protocol sites, post-open locator/route/revision, commit and explicit Unknown. Failed prerequisites and failed commit/open paths each increment once. Existing LocatorAttempt/Opened/Validated/Committed/Error and quota behavior remain unchanged. Diagnostics disabled leaves all new counts zero; no revision/pair/lease authority comes from the counters. Late commit/storage errors are intentionally grouped as Commit; this is not a general tracing facility.

The listener retains the old aggregate ListenerTlsError semantics and adds ListenerPrerequisiteRejected, ListenerActualTlsError and ListenerDeadline. The last category identifies an outer15s timeout; it does not infer whether owner lookup or TLS was pending. The first two count actual failures at their respective await sites; TLS is not attempted after prerequisite failure. Admission order/deadlines are unchanged.

The bounded host reader allows the fixed numeric histogram and listener keys. It keeps the existing file16KiB, object64, array64, depth8, marker/nonce/process/instance and immutable-record constraints. Counter and histogram objects fit their caps. No arbitrary text/key is exported. The diagnostic manifest now pins the protocol source and the new checker, and requires the new isolated base7a8772b0 so stale image/source manifests fail validation.

## Verification

Review root: `.tmp/terminal-t1-3e/discovery-rejection-review-01`. The detached source checkout is at7a8772b0 with only this task's Rust changes. Foreign discovery/migration tests and other dirty files are excluded from compilation and preserved in the main tree.

`focused-02`:4 tests passed (2 CLI,2 protocol), stable Windows EXEs, NativeToken, Idle/jobs1,900s work/20s cleanup reserve,113.735s elapsed, no timeout. The owner regression compares diagnostics off/on failures and proves unchanged peer-locator bytes/revisions, repository generation, closed pair and no leases for stale generation, clock-provider failure, malformed frames and quota exhaustion. The histogram test checks saturation, opt-in behavior and bounded JSON. `focused-01` failed after1 passing test because the new test expected the error text without its existing `OD2 ` prefix; the fixture also needed increasing generations for its second iteration. The test corrections and initial logs/source snapshot are preserved, not repaired in place. The first failed executable was not separately frozen before later stable-path replacement; its sources/logs, not a historical EXE hash, are the retained proof.

The actual host-reader regression passes: two valid records (new histogram and older record) accepted, three forged records rejected (arbitrary text, unknown key, wrong runtime binding); forbidden contents do not appear in its output. Existing LAN codec regressions, locked/offline CLI/bootstrap image build, syntax/format/whitespace checks and final image/source pins are recorded in the final review package checks. Unchanged full application suites and previously limited broad Clippy are not claimed as repeated coverage.

## Proposed product change for separate review

No product fix is implemented. The narrow candidate to review is a bounded deferral of an expected-sender, signature-authenticated announcement rejected because `issued > local.now`: retain at most one759-byte encrypted datagram per expected peer, wait at most an architect-approved small monotonic budget, then retry through the existing complete owner path when local wall time reaches the signed issue time. Initial signature verification does not establish that later body/consent/locator checks would pass; they still must run before any import. Keep the signed expiry/duration, locator/consent intervals, profile/network observation, high-water, quota and admission checks. Do not extend expiry or trust peer time as the local clock. A newer arrival must not indefinitely reset the pending deadline. The cap, cancellation, replacement/rollback/expiry behavior and denial-of-service implications require explicit architecture review before coding. Merely removing `issued <= now` would change security policy and is not proposed here.

The field category can establish whether this candidate addresses the observed fault; other categories require their own diagnosis. No system time synchronization, clock setting or cryptographic/identity acceptance relaxation is part of this slice.

## One proposed field observation, still gated

Concrete package: `.tmp/terminal-t1-3e/discovery-rejection-review-01/package/runtime-plan.json`, exactly one fresh `startup-diag-03`, using the existing ordinary A/B agent incarnations and stable bin paths. Fresh Setup credentials only; no failed originals. Native work800/whole815 seconds, reserve60, controller1800/outer1860. Existing45/60/30 product startup limits, harness connect85/probe60/minimum call2, original first-connect90s and eligibility-gated separate diagnostic90s retain their reviewed semantics. No messages/offline/restart campaign. Capture typed rejection/listener counters and normal correlated startup/final records, then graceful owner/native cleanup and restoration of the six original inputs. Reuse the reviewed transport/correlation/deadline/cleanup helpers.

Default wrapper action is local validation only. A new separately hashed root acceptance must bind source/inventory/plan/commit before any `--run`; neither this source submission nor the general testing authorization is treated as live deployment approval under this delegated contract. No install, UAC, firewall/profile/VPN/account/system-time change, remote transfer or new live process has occurred. Submit one callback with publication/source/reproduction/package evidence, then idle.
