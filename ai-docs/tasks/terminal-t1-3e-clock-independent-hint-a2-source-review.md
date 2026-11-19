# Clock-independent Auto-LAN hints — A2 source review

2026-11-20. Implements the accepted [A2 task](terminal-t1-3e-clock-independent-hint-a2-task.md) and [clock contract](terminal-t1-3e-clock-skew-contract.md). Source candidate only; A2 awaits architectural acceptance/publication. No laptop deployment or field attempt, system clock/network policy change, new wire handshake or consent-policy change.

## Published prerequisite and candidate

A1 exact3 was committed and pushed as **436554e7ed871bf056f671252b689c671280d670**, parent **b6a5715ef381e492599f53796e7bd48790b09ceb**. Publication proof `.tmp/terminal-t1-3e/clock-independent-hint-a1-publication-01/publication-final.json`, SHA256 `24708476a52b7ca413ce74f4ec6451eb31f91f7a9cb2d64779f9b12ba270ec16`; accepted allowlist SHA256 `611fc6d1b74150b411d903e8f8692728f2f20b5cf0eb614d3906e7f2d067faea`. Parent, exact committed paths/blobs, empty index and successful live origin result were checked at publication. This does not claim A2 publication.

A2 review/evidence root: `.tmp/terminal-t1-3e/clock-independent-hint-a2-review-01`. `allowlist.json` identifies task-only accepted raw hashes/Git blobs; `source.diff` is based on A1. The isolated `source` worktree supplies the tested candidate. **`discovery_tests.rs` is mixed:** main working bytes include a pre-existing unrelated history test; accepted/tested bytes exclude precisely that preserved insertion. Use the isolated accepted blob or task-only patch when staging; broad `git add` of the main mixed file would include foreign work. `before.json`, `foreign-before.patch`, original backups and final checks identify this boundary. Other foreign tracked files and unrelated untracked notes remain excluded. A2 has no commit or push.

## Behavior and gates

`SocketWorker` captures local `Instant` and generation immediately after receive, before peer filter/inbox delivery. Bounded inbox4 and validation8/s remain. Owner processing consumes the same45s deadline; receipt/dequeue at the deadline is rejected before a state write. The authenticated-hint decoder preserves signed locator bytes. Remote issue/expiry and peer-locator dates no longer act as receiver calendar gates in Auto mode.

The new typed `RuntimeOperation::AutoLanPeerHint` is appended to preserve existing serialized operation discriminants. It shares signature/scope/revision/conflict/storage handling with Locator, with a named internal purpose. Owner requires the matching Auto purpose, live network token, current actual consent, real local wall/high-water, and monotonic deadline at the closed staged gate. Manual peer and own Locator operations retain their strict calendar checks. Stored locator shape/schema and signed dates are unchanged.

Both contexts are verified before displacing a pair. Imports retain durable per-context revision knowledge; final exact-pair reread opens targets only after both match the current scope. A partial write followed by failure/recovery leaves the pair closed. Duplicate identity includes current network/policy/own endpoint/locators and exact signed peer pair/refs/revisions, not HPKE randomness or announcement issue time. A duplicate does not write state, move an active deadline, reset watches/backoff or renew leases. One retained pair has a fixed45s window; only a later locally eligible receipt after the window plus15s cooldown may schedule another bounded **untrusted** window. Restart restores no admission.

Targets are watched per context. On candidate expiry, unadmitted targets/pending admissions are withdrawn; a matching healthy lease keeps its context and original lease limits. The other context still needs its own active candidate and fresh admission. Outbound selection and listener source selection capture the source deadline before TLS awaits. Existing30s admission and15s TLS bounds are clamped to it; permits keep that captured deadline across subsequent retries and check exact context, connection, pair, network and local policy/high-water after awaits and at staged Root commit.

The existing TLS exporter-bound Hello/Proof/Admitted path remains required. Hint dates are ignored only for peer transport metadata in the matching Auto pair; own locator/current consent stay strict. Lease lifetime uses local cap/current consent/own locator/local scope, retaining peer-calendar limits for manual admission. Fresh verified Hello may learn a monotonic Root snapshot before full admission, including a snapshot revoking the Hello signer. A hint alone imports no Root. Known revocation removal, rollback and equal-revision conflict remain rejected, including after repository reopen.

## Focused evidence

All local execution uses fresh synthetic profiles, the stable Windows harness, NativeToken, Idle priority/jobs1, serial tests and bounded900s work/920s cleanup envelopes. Commands, per-attempt source snapshots, stdout/stderr and result receipts are preserved. Failed `focused-02` (4 passed/1 failed) is retained: test Proof sequencing was wrong for the responder (`A Proof` missing); only the test sequence was repaired. `focused-01` was the earlier two-test increment, not final coverage.

Final selected results and artifact/source pins are recorded in `checks.json` and `artifacts.json`. New owner/state/admission tests cover:

- Both receiver roles and15 sender offsets each:0,±1/3/5/10/60s,±1h,±24h; actual typed state commits use unmodified local wall/high-water.
- Receipt/dequeue45s boundary, freshly encrypted same-pair wrapper, fixed active budget, cooldown and locally scheduled replay without admission.
- Invalid later consent slot, stale/equal-conflicting locator revisions without target displacement, closed two-context partial write, durable replay protection after actual repository reopen; true consent expiry, local wall rollback and strict manual locator import.
- Real loopback TLS/exporter plus signed peer Hello/Proof/Admitted through actual receiver owner methods; an already expired peer locator is transport metadata in Auto. Healthy first lease survives candidate expiry; second admission is denied; duplicates/scheduled replay do not extend the lease.
- Actual permit check after a timed await, wrong TLS-bound Hello rejection, verified Hello Root learning, network source withdrawal at staged Root commit, signer revocation learned before admission, revoked signer denial, persistent Root rollback/conflict/revocation-removal rejection.

Selected existing regressions cover partial-pair recovery/policy changes, send-time publication checks, post-await/staged source revocation, transport shutdown/backoff, no-network local IPC, rejection accounting, actual wrong-path authority denial and manual admission/lease/endpoint behavior. See logs for actual matching counts; zero matches are not coverage.

Final source snapshot is `attempt-05/source-pins.json`. `final-focused-02`: **7 passed**,0 failed,69.75s test execution/139.406s bounded command. `regression-02`: **11 passed** across six selected invocations (6+1+1+1+1+1),112.328s total. Both exited0 without timeout. The same stable CLI library harness is used; final executable SHA256/size/path are frozen in `artifacts.json`. These18 distinct tests are the focused final coverage; earlier incremental runs are not added to that count. Rustfmt check, isolated task-path/whitespace checks and reverse application of the exact candidate patch pass in final packaging.

Scoped production strict Clippy (`kilogram-state` and `kilogram-cli`, `--lib --no-deps --locked --offline -- -D warnings`) passed in `clippy-02` (18.0s, exit0). `clippy-01` retains two new-code findings (nested conditional/type complexity); both were fixed without lint suppression. This is not an all-target lint claim. `regression-01` retains six passing tests followed by a stable-image Copy-Item sharing failure before its next selector; later native process readback found no remaining harness. It is a runner/environment failure, not a failed test. The retry adds a bounded2s gap between stable runner invocations; no product/process policy was changed.

## Limits and next boundary

These are local owner/state/admission tests. Synthetic private-LAN advertisements are mapped to real loopback connections by a narrowly `cfg(test)` path adapter that still checks DirectOnly, signed LAN shape, endpoint IDs and selected loopback path. Production continues to call the exact existing LAN path verifier. Peer signed admission frames are supplied by test control, not a two-host wire roundtrip. No physical NIC, Windows firewall, real socket-to-owner queue latency, suspend behavior, arbitrary OS clock jumps or field progress guarantee is claimed. Admission/network-await cancellation and cleanup have focused local coverage, not a new field result.

Actual absolute consent validity and sender own-publication policy remain tied to real local wall/high-water. A device with an invalid actual permission may still refuse publication/admission even though hint discovery is independent of peer dates. No receipt-based consent renewal is introduced.

Submit this exact candidate/evidence and A1 publication proof once to the architect, then idle. The architect decides A2 acceptance/publication and any subsequent two-host verification; existing lab agents/data/settings remain untouched.
