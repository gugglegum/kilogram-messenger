# Discovery hints without remote wall-clock dependence

2026-11-20. **Contract proposal only; no product code/build/live run.** The [latest user direction](terminal-t1-3e-clock-skew-contract-task.md) supersedes the earlier bounded-skew/deferral/cache defaults. Recommend symmetric receiver-local hint lifetime and the **existing** fresh OD2 handshake. No waiting for peer issue time, trusted peer-clock estimate or OS time adjustment.

Accepted diagnostic03 note published as **62bd2c2299acd716d980e7f8060a13991cb6dec0**, parentc835e3e0ac08767a4d14d7cebcb5261a30c706fc, exact1/push/origin verified. Proof `.tmp/terminal-t1-3e/discovery-rejection-note-publication-01/publication-final.json` SHA256 `b7469d32e8f16fbcab111f54fe244d2c9a1591d004c5af04b24e239e08b03e4f`. No further unchanged diagnostic attempt.

## Meaning and threat model

The [original T1.3 contract](terminal-t1-3-contract.md) already calls the announcement a signed socket hint, allows replay within its window, and says its expiry must not end a healthy lease. Current45s is a sender implementation choice, clipped to consent/locator dates;60s is the codec's maximum signed span. Neither is cryptographically required. Absolute packet-age checks are not live-possession proofs.

Recommend initially keeping **45s from actual local monotonic receipt** as unverified-hint retention/attempt eligibility, avoiding unrelated TTL tuning. It is a bounded resource knob, not creation freshness or authority. Each connection attempt retains its existing30s cap. Signed1..60s duration may remain a v1 shape check, not a local deadline. An apparently hours-old packet and a future-dated packet are treated symmetrically: either can suggest an address; neither proves its owner is alive now.

| Threat | Required defense |
|---|---|
|Unknown/spoofed peer, recipient/context/role/address substitution|Existing HPKE, pinned/current known credentials and revocations, signatures, both context/consent refs, signed locator pair and exact LAN source/route.|
|Old endpoint or revision replay/rollback|Persistent per-context locator high-water and exact equal-revision bytes. A duplicate cache/receipt time cannot replace this, including after restart.|
|Recorded signature mistaken for live possession|Existing fresh TLS exporter binding (ALPN/endpoint pair), signed Hello nonces and scoped Hello/Proof/Admitted transcript; no lease from discovery alone.|
|Replay/flood extends work or permission|Fixed active first-receipt deadline; duplicate key ignores wrapper/ciphertext/issue time; existing inbox4, validation8/s, concurrency/backoff; one candidate pair for the configured peer.|
|Pre-admission Root mutation|A separate fresh TLS-bound Hello, independently Root-signed monotonic snapshot, pinned account and final network/source permit, as detailed below.|
|Expired/not-yet-active actual authorization|Explicit current permission checks. Do not restart consent lifetime at receipt or claim all time dependencies disappeared.|

## Minimal receiver/state contract

1. Preserve signed bytes. Capture monotonic receipt `m` plus network generation at the socket, not owner dequeue. Validate759-byte format/padding, recipient/signature, installed identity/context/reference/route, both locator signatures and revisions. **No sender-issued/expires or peer-locator-date comparison with receiver wall time for a transport hint.** Keep safe numeric interval shapes and structural relationships within the sender's announcement/locators. Remove numerical announcement-to-consent interval containment as a hint freshness check: sender creation time is not the grant's policy clock. Exact consent reference and actual current permission remain separate checks.
2. Candidate key: peer/certs, both contexts/consent refs, both locator IDs/revisions, endpoint/socket, local policy/network/owner incarnation. A new HPKE wrapper or issued timestamp around the same pair is a duplicate. At most one candidate; fixed deadline=`m+45s`. Queue delay consumes it. Duplicate input cannot reset first_seen, active deadline, backoff, transcript or lease. Valid higher-revision replacement uses the existing closed pair gate; rejected input cannot displace it.
3. Preserve two-slot import, durable revision commits and final exact-pair reread. Add an explicit **Auto-LAN hint import purpose** to the closed state gate: enforce signed scope/revisions/current local permission, real captured local wall high-water and local monotonic source/network permit, while removing peer-wall validity from this purpose's Locator gate. Do not broadly delete manual/own-locator validation or write fake fence.wall. Retain raw dates as signed data, not received_at. Codec-only change is insufficient: `discovery_scope`, pair/permit, state Locator checks and repeated peer-locator checks in `check_lan_admission`/`begin_admission` all need the same purpose distinction.
4. Recheck policy/network/endpoint/local deadline before use and after awaits. An unverified attempt ends by both its existing30s limit and candidate deadline. On expiry without a live verified connection, drop the unverified target/cancel pre-admission use. A later replay, including after eviction/restart, **may trigger another locally rate-limited untrusted attempt**. This is not renewed trust: every attempt still needs a fresh binding/proofs/current permission. The scheduler/backoff, not each received duplicate, authorizes retry work. Durable revision knowledge survives cache loss.
5. An admitted context's healthy lease is governed by current policy/network and its own deadlines, not hint45s. Keep verified pair/endpoint identity without letting duplicate hints extend leases. One healthy context does not authorize the other: its own admission remains required. Hint expiry must neither close the first healthy context nor grant the second. Restart restores no live admission; gate closed, saved locators unproven, durable revisions retained, fresh handshake required.

These are receiver-local resource semantics, not a claim of recent creation. Far-future expiry cannot install a far-future local deadline. Older signed hints may cause bounded stale dialing, deliberately accepted as a tradeoff for clock independence. No new handshake, sender cache, timestamp rewriting or cryptographic nonce reuse.

## Pre-admission Root update is an authoritative exception

The [T1.3d result](terminal-t1-3d-result.md) explicitly permits a verified Hello to import Root **before full admission**. “Nothing authoritative happens before admission” is false and is not a premise here.

Preserve the existing chain: exact TLS endpoint/pair/LAN source permit → `Credentials::decode` verifies certificate/authority Root signatures and exact account encoding → Hello context/role/pinned certificate reference → device-signed frame bound to the current TLS exporter → `import_authority_with` checks pinned account, monotonic revision/equal-byte conflict and preservation of known revocations → final `RemoteAuthority` permit check in the closed commit gate. Permit binds this connection/pair/network and local attempt deadline. Discovery bytes carry no Root snapshot to import.

`decode` intentionally differs from `parse`: a genuinely newer Root snapshot revoking the Hello signer may still be learned, while current-policy checks then deny that device's admission/data. Do not move revocation learning behind successful full admission. Old Hello on another binding fails; old/conflicting Root fails durable checks after restart too. A new valid Root snapshot may update policy through a fresh verified Hello even if the hint was old; authority derives from the Root signature, monotonic pins and verified source, **not** received_at. Test final source revocation at staged Root commit and denial of the now-revoked signer. No hint-only key/message/root authority is introduced.

## Time-dependency inventory and scope limit

| State/time | Decision |
|---|---|
|Remote announcement creation/expiry; peer endpoint locator dates|Transport metadata in the new Auto-LAN hint purpose; no cross-wall validity gate or deadline. Numeric shape/signatures/pins/revisions remain.|
|Own locator/publication send authorization|Local-clock/current endpoint/network/revocation checks stay. Own clock steps may revalidate or block publication; no receiver clock is consulted.|
|Root-cosigned consent not_before/expiry|Actual absolute permission policy: `policy::current/check_time`, sender scope, commits, admission and lease end still use local wall. **Consent clock mismatch can therefore still block publication or admission.** No receipt-time restart or hidden expiry extension.|
|Device certificate/Root authority|These credentials use account/device/capability/authority-sequence/revocation/revision, not an announcement-relative45s lifetime. Keep known-current authority/pins; discovery does not prove globally latest Root.|
|Lease/source permit/network freshness|Local monotonic caps and current policy/network gates. Remove only the peer hint's absolute-date authority in this path; actual consent remains authoritative. Healthy lease does not depend on hint cache retention.|
|Local high-water, steps, suspend/restart|Store real commit wall. Rollback below persisted floor stays ClockBlocked; forward jumps may expire actual permission. Monotonic caps prevent rollback extension. Revalidate network/owner scope after expired proof/suspend; test clocks that do/do not count suspension. No learned peer clock.|

This is **discovery/transport clock independence**, not universal wall-independent authorization. Symmetric minute/hour offsets must have the same hint eligibility; any later inactive/expired permission is reported separately. Sender publication currently also depends on local consent validity, so no claim that both devices always publish/admit with arbitrarily wrong calendars. Making authorization itself wall independent requires a separate explicit signed activation/revocation-epoch or durable duration policy; replacing consent expiry by received_at would permit replay-driven renewal, especially after restart. No guaranteed real-time expiry can be derived from untrusted wall clocks alone. This remaining architecture decision is exposed, not silently implemented.

## Alternatives and progress

Bounded±10s tolerance leaves a cross-clock cutoff and downstream consistency requirements; it is superseded as the main design. Deferral and aged sender retransmission still wait approximately the skew. Sender-cache audit found own-locator/authority/current-consent/recovery/network/stop invalidation, while peer-locator commits leave publication valid and policy_generation unchanged despite advancing DB generation/high-water; keying cache to every commit could renew forever. A complete cache would also need credential/consent/locator/route/owner/network identity revalidation and repeat-ciphertext privacy analysis. None is needed for the recommendation.

No skew-proportional delay. As a conditional scheduling example, search transmissions <=6s apart, <=1 consecutive loss and <=2s delivery/owner processing yield candidates within14s after both endpoints are ready and locally permitted, independent of offset. This is **not** a messaging bound: connected14–16s cadence, startup,30s admission attempts and retry/backoff retain separate bounds.45s limits retained unverified work; it is not user waiting time. Unlimited loss/churn has no progress guarantee.

## Required focused matrix after implementation

| Cases | Expected |
|---|---|
|Both directions/roles/slot orders;0,±1/3/5/10s,±60s,±1h,±24h sender dates|Same immediate hint eligibility. Label separately any absolute-consent policy refusal; no blanket E2E claim.|
|Apparent past>45s/future/far-future expiry; invalid duration/u64 shape|Past/future symmetric local budget; no raw timestamp rewrite; malformed shape rejected.|
|Receipt→dequeue at45s boundary; repeated ciphertext or fresh wrapper of same pair|Queue time consumes budget; deadline does not slide; no active attempt/backoff/lease reset.|
|Eviction/restart then replay; old/conflicting revision; reordered/new pair|Durable rollback protection; at most new untrusted scheduled attempt, never restored admission or Root from cache alone.|
|Dead peer/old endpoint secret; Hello/Proof replay on wrong TLS binding/nonces/context|Bounded failure; no data permission.|
|Fresh verified Hello with new Root (including signer revocation); stale/conflicting Root; source revoked at staged commit|Pre-admission monotonic Root learning preserved only through verified source; now-revoked device denied; stale/revoked source cannot commit.|
|Signed but invalid later context/consent ref/locator/route/recipient; unknown/revoked key|Rejected without target displacement or revision advance.|
|Policy/key/network/own-locator change at receive, either commit, TLS await or Root staging|Closed gates, current final permits, partial pair remains closed; old epoch cannot commit.|
|Hint expiry with one/both healthy contexts and another unadmitted context|Healthy lease retained under its own policy; no second-context grant or replay lifetime extension.|
|Local wall jumps, true consent boundary, suspend/resume, shutdown/restart|Explicit remaining authorization failures, real wall fence/high-water, monotonic caps and clean owned resources.|

Small local model: `.tmp/terminal-t1-3e/clock-skew-contract-review-01/hint-lifetime-model.py`. Arithmetic/state-model evidence only, not Rust/crypto/transport coverage. Earlier bounded-skew draft/model are archived as superseded. Reuse current synthetic credentials and stable Windows harnesses for implementation tests after review; no new field test for this contract.

Source map: `apps/kilogram-cli/src/ordered_dialog/runtime2/{discovery,lan_runtime,owner,policy,fresh_consent,admission}.rs`; `crates/kilogram-state/src/vault/indexed_dialog/runtime2_ops.rs`; protocol `ordered_dialog_v2/{lan_discovery,runtime_locator,mode}.rs`; transport `ordered2.rs::binding`; `apps/kilogram-cli/src/ordered_dialog/handshake.rs::Credentials::decode`; the historical contracts linked above. Submit contract/publication/model once, then idle. Existing agents/data/system policy untouched.
