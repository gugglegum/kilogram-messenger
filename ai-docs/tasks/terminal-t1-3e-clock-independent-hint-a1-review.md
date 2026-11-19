# Clock-independent hints A1 — pure codec review

2026-11-20. Implements only [A1's protocol boundary](terminal-t1-3e-clock-independent-hint-a1-task.md). No owner/state/permission/admission integration, field deployment or executable package. New A1 code awaits root review before commit.

## Accepted contract publication

Published **b6a5715ef381e492599f53796e7bd48790b09ceb**, parent **62bd2c2299acd716d980e7f8060a13991cb6dec0**, exact1 accepted contract, normal push success. Raw/blob/pathset/parent/index/live origin and foreign work verified. Proof `.tmp/terminal-t1-3e/clock-skew-contract-publication-01/publication-final.json`, SHA256 `d782b426fddbbfbf6853603674225ef40c85f6698f934f758cf03a71cb10f37c`.

## API and shared validation

`lan_discovery` adds `HintContext`/`HintScope`, `AuthenticatedHint`, and `open_authenticated_hint`/`open_authenticated_hint_observed`. HintScope contains only expected sender/recipient Root-verified credentials, two exact context/consent-reference bindings and sender IPv4. **There is no receiver wall time or consent calendar input**, so the hint path cannot achieve acceptance by supplying a fake local now.

AuthenticatedHint has private storage and read-only accessors for signed locators in local slot order, socket, original signed announcement interval and exact759-byte encrypted datagram. Dates and locator bytes are preserved. It does not convert/deref to legacy Candidate, nor offer state-write, Root-import, current-permission or admission capabilities. Its authentication claim is scoped to the caller's supplied expected credentials/references, not knowledge of globally latest authority or actual liveness.

Both public API families call the same private `open_verified` implementation: one envelope parse/HPKE open/padding/signed-body/device-signature verification, followed by one shared locator/body pipeline. A private named `ValidationPurpose` selects StrictWindow versus AuthenticatedHint calendar semantics; no public skip-checks boolean, second decrypt or parallel parser. Both purposes always retain exact identity/reference binding, wire/body version/shape, signed duration1..60 with checked subtraction, both signed locator scope/signature checks, locator containment of the announcement interval, exact configured IPv4 route and endpoint/socket pair consistency.

Only the hint purpose omits receiver-wall issue/expiry comparisons and numerical announcement-to-consent-calendar containment. StrictWindow retains the previous short-circuit issue/expiry/duration check, exact error text/failure categories and consent interval containment. `seal`, `open`, `open_observed`, their public signatures and Candidate output remain strict. The wire v1 envelope, signed fields, padding and cryptographic domains are unchanged.

All production call sites remain unchanged and continue to use strict APIs. No consumer outside the protocol implementation/tests references the new API/types. This is intentionally dormant until coherent A2 integration; **the field fault is not yet fixed in the active application**.

## Actual focused tests and provenance

`.tmp/terminal-t1-3e/clock-independent-hint-a1-review-01/source` is a detached checkout of b6a5715e with only the two A1 Rust files copied in. Main-tree foreign discovery/migration changes are excluded. `attempt-01/source-pins.json` freezes those exact sources; no failed source-test iteration occurred.

`focused-01`: **12 tests passed**:5 new authenticated-hint unit tests,2 existing strict discovery-rejection unit tests,5 existing LAN codec integration regressions. Other protocol harnesses selected zero tests; no unrelated application suite or benchmark. New tests use actual existing synthetic credentials, HPKE, signatures and encoded datagrams, not the earlier contract model:

- 60 immediate hint cases: both sender directions/roles, both receiver slot orders,0/±1/3/5/10s/±60s/±1h/±1day sender timestamps. Sender sealing uses its own synthetic calendar; HintScope never accepts a receiver clock. Socket/context/slot mapping, signed locator bytes/dates, original announcement dates and exact encrypted datagram match.
- 9 envelope/identity failures: length, local recipient key, wrong encrypted recipient, altered ciphertext, wrong signer, wrong peer scope, duplicate context, outer version and nonzero padding. Error and single fixed observation agree with the no-observer hint entry; representative crypto errors match strict entry behavior.
- 10 failures after a valid outer signature: wrong consent reference/context/route, malformed/oversized locator, bad locator signature, locator start/end not containing the announcement, disagreeing sockets and malformed body shape. A signature-authenticated wrapper cannot bypass later checks.
- 5 signed duration failures: zero, reversed,61s, unsigned wrap candidate and huge span. Valid1s/60s and cases ending at u64::MAX preserve dates without overflow.
- Legacy issue−1/issue/expiry−1/expiry and extreme receiver clocks, plus three invalid consent calendars, retain their exact old decisions/errors; the hint path accepts the same structurally sound bytes without claiming permission. Existing strict2 and integration5 also pass unchanged, including replay/domain/canonical/budget checks.

Windows execution used `scripts/run-cargo-tests-stable.ps1`, compile `--no-run`, locked/offline profileverification, stable paths, NativeToken, Idle/jobs1.600s work/20s cleanup reserve; elapsed59.703s, compile49.52s, no timeout/forced cleanup. Tested stable artifacts are frozen with hashes in the review folder. No Rust harness was launched directly with cargo test. Syntax/format/whitespace, source/raw/Git-blob allowlist and unchanged production-callsite checks are in the review receipts. No broad Clippy green claim or new CLI/bootstrap image build is made at this pure-codec boundary.

The isolated checkout's PowerShell helper may have checkout-normalized CRLF while the main copy has LF. `unchanged-production.json` records both raw hashes and verifies identical normalized Git blobs and content apart from line endings; the actual tested checkout helper is pinned separately. The two changed Rust inputs match their frozen raw hashes exactly.

## Remaining coherent A2 work

1. Real socket monotonic receipt and network generation, fixed45s local candidate budget, duplicates keyed by exact locator pair/references, bounded independent retry after eviction/restart; no timestamp-derived deadline.
2. Typed Auto-LAN owner/state hint-import purpose, real wall/high-water, persistent revision/conflict checks and closed two-slot commit/recheck. No generic removal of manual/own-locator/actual permission checks.
3. Replace **all** downstream peer-hint temporal gates coherently, including pair/source permits, repeated admission locator checks and peer-date use in lease calculation; no partially enabled mode.
4. Distinguish each context's pending attempt eligibility from a healthy verified context/endpoint. Hint expiry cannot terminate a healthy lease, extend one on duplicate or admit an expired unverified second context.
5. Preserve fresh TLS exporter/Hello/Proof/Admitted binding and pre-admission Root learning only through the verified connection-bound final source permit, Root account/revision/conflict/revocation protections. Test staged permit/policy/network invalidation and signer revocation.
6. Keep actual consent interval, local publication and durable real-wall limitations explicit. This codec cannot resolve wall-independent authorization and must not restart consent TTL at receipt.

Submit the exact3 source/test/review-note diff and focused evidence, then idle for review. No A1 code commit or A2 implementation is authorized by this submission itself. Existing agents, network rules, test fixtures and system settings remain untouched.
