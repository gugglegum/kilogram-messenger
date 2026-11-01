# S5.3 mapping revision1: B-only initiation and local cancellation

2026-10-26. Narrow response to [review1](long-lived-dialog-a3-migration-s5-handoff-plan-review-1.md),
baseline `64c2359788ead360314793e47741adacb83b8d4f`.
The original [mapping](long-lived-dialog-a3-migration-s5-handoff-plan.md) remains
34046 bytes/SHA256 `d7b4faf56b11ad320311348714b286a9b1da77d8c17cc5401604a3f6f57c4216`.
The accepted S5 contract remains unchanged. This addendum replaces only the
incomplete initiation/cancellation mapping and the F estimate below. No executable
budget or implementation is activated by this submission.

## 1. Existing notification, explicit trigger

Use unchanged [FrameKind::OpenIntent=4](../../crates/kilogram-protocol/src/ordered_dialog_v2/frame.rs#L5)
and its [six-field body](../../crates/kilogram-protocol/src/ordered_dialog_v2/frame.rs#L314):

```text
[C, 3, [Offer1_id], [Grant1_id], [A0_id], [B0_id]]
```

Here3 is complete Native phase, not wire Prepared. All four options must contain
the exact immutable old IDs from the verified existing scope. The wire codec's
phase/optional-width acceptance alone is insufficient. Require a freshly admitted
Origin::Existing ALPN2 lease, actual binding/endpoint/route, signed sender B/cert
role1, matching original C/D/E/SID/cert roles and complete Ready1/genesis, correct
local stage and current Root/Consent2/lease at staging and final gate. Reject
wrong family/sender/scope/phase/absent or mismatching refs before trigger/head/
cut/debt/Native changes. No field, coordinator or Q is added to the wire.

IPC3 upgrade on B durably records its local request16/intent and wakes B's owner.
On an admitted existing lease B opens its own bounded bidirectional RPC carrying
this OpenIntent; A's incoming dispatcher must handle it even though A initiated
the TLS connection. Hello/ModeView/Resume never stand in for this notification.
A with no local intent does not send OpenIntent merely to poll B. It can exchange
the ordinary existing Resume/Head while the notification window is open.

B sends once per fresh admitted opportunity while its intent is pending, with at
most one notification RPC in flight per C. A newly committed B intent during an
open opportunity wakes that same bounded path. There is no fast retransmit loop
inside a failed opportunity. Across reconnect, B retains request16 and the same
six-field body but signs a new Frame2 for the new TLS binding/current lease. The
old full Frame2 is not replayed across exporters. B continues future notifications
until it durably accepts P/Prepared or locally cancels; a socket write/Complete/
timeout alone cannot mark the request fulfilled.

## 2. A-driven opportunity without starving L1

Extend the [A-only dialer](../../apps/kilogram-cli/src/ordered_dialog/runtime7/network.rs#L215)
with a per-C scheduler; B does not become a TLS dialer/coordinator. Eligibility:
enabled role A, actual complete existing scope, current valid existing Consent2,
valid own/peer Locator2, eligible actual route and no policy/fork block. When A is
L1 with no trigger this only arms discovery; it creates no intent/P/cuts/Q/freeze.

Use these proposed monotonic scheduler constants:

| Constant | Exact meaning |
|---|---|
|initial discovery due=5s|After becoming eligible or restart, arm the first discovery; the protected ordinary-L1 turn below still has priority. Consent/locator presence is not an immediate lease switch.|
|discovery interval=60s|Minimum start-to-start interval for no-trigger discovery attempts per C, measured from actual attempt start. Repeated IPC/import/wake calls do not reset it or create more attempts.|
|probe whole window=45s|From beginning lease handover/permit acquisition through TLS, Hello2/Proof2/admission and notification/closed handler, while no durable P exists. This allows existing whole admission30 (including TLS15) plus one15s notification round. Steps use remaining time; neither existing timeout is extended.|
|quiet window=15s|After successful admission listen for B's OpenIntent for up to15s, capped by the remaining45s. No trigger closes the probe; it does not become an indefinite ALPN2 data lease.|
|ordinary protected turn:30..90s|After a no-P return, give eligible genuine L1 at least30s of scheduling opportunity and one completed bounded ordinary work cycle, or its actual failure/deadline. The upper turn allowance is90s: admission<=30 plus at most Resume/one Prefix/one Deliver/one ACK RPC, each<=15. Do not cancel a progressing cycle for a due probe.|

Use monotonic time, one scheduled attempt per C, round-robin C service and the
existing global permits before TLS/spawn. Waits and permit acquisition count in
the45s. At the boundary, invalidate/close the previous ALPN1 token and finish its
owned network task before admitting ALPN2; never hold two admitted leases for C.
Other C retains its normal shares and may progress throughout.

On no trigger or pre-P timeout/refusal, finish/cancel the probe generation and
ask the serialized owner for actual durable mode after any in-flight commit has
resolved. If still L1, close ALPN2 and run genuine ALPN1 admission/current Consent1
and lease gates when eligible. The ordinary protected turn must finish before
the next no-trigger probe. Check due time between closed RPCs and start no new
unprotected L1 cycle once discovery is due; an already in-flight RPC retains its
existing15s bound. Thus slow but successful L1 work cannot be repeatedly cut off
at the same admission/Resume position. Fast/idle cycles do not spin: normal L1
stays admitted until the due time/protected floor; the60s interval still applies.
If L1 permission is absent, report L1 AuthBlocked rather than claim an ALPN1
admission or progress; keep bounded Consent2 opportunities if independently valid.
An actual L1 admission/RPC failure ends that protected attempt and is observable,
not a reason to wait forever for a nonexistent successful turn.
Actor waits/preparation count in the window/turn allowances. Expiry stops new
work and expires the opportunity token checked by pre-P final gates; it is not
permission to abandon an in-flight durable operation or guess its outcome.
Resolve/poison-reopen first, and let an actually committed P win over a stale
timeout. Existing Native trial/commit outcome rules remain unchanged.

A local upgrade request or already accepted peer trigger wakes ordinary control
work immediately at a closed-frame boundary. It uses existing bounded admission/
control deadlines, not an endless discovery lease. A failed pre-P control attempt
retains its outcome/trigger and returns an ordinary protected L1 turn before
another attempt if L1 is eligible. Repeated request IDs cannot continuously
preempt L1. No policy failure downgrades durable state or bypasses current gates.

If P commits during the window, the owner now observes Offering and cancels only
the no-trigger discovery scheduling rule. It does not extend the admitted lease's
original expiry. Existing control recovery continues on that lease or a fresh
ALPN2 lease under ordinary deadlines. A stale probe timer may close only its own
generation; it cannot restore ALPN1 after P or close a successor lease. Prepared B
likewise never returns to ALPN1, including lost replies, expiry, no Q and restarts.

Under healthy/eligible peers the protected cycle can delay a due probe beyond60s;
that deferral is bounded by its90s turn/RPC deadlines, not arbitrary queue drain.
This is an explicit control-latency tradeoff, not a network-availability guarantee.
Scheduler tests must exercise both fast and deadline-sized cycles. On reopen,
rebuild local timers with the initial delay and protected turn; persist intents/
outcomes/stages, not a wall-clock promise of when another machine received them.

## 3. Closed A handler, coalescing and status

Add a paid bounded `AcceptPeerUpgradeIntent` purpose to the same Root7 existing
writer. After full checks, it persists the first accepted B device/request/body
digest for this C before replying or scheduling P. It changes neither LocalMode
nor cuts/native/freeze. Capture current root/policy/head in the closed operation;
on clean final-gate failure no accepted trigger is invented. After success, a
crash before P leaves a real durable trigger for A's next fresh admission.

Only A executes Propose. A's own intent, the accepted B trigger, or both coalesce
into one per-C transition. Read durable stage/request outcomes before signing:

- L1 plus no trigger: ordinary operation/discovery only, never P.
- L1 plus own/peer trigger: current-gated Propose commits the single saved P and
  its selected consent/cut before publication.
- Offering or later: a valid repeat/coalesced notification returns the saved P
  or its already-durable continuation/status; it never creates a second P,
  replaces its selected consent, re-cuts or invokes Native.

The response to B's valid OpenIntent can be existing FrameKind::Propose carrying
the exact saved P and selected-consent bytes, only after durable commit. If P is
deferred, use a bounded existing Error/status outcome; B still retains pending
intent and does not treat that as Prepared. Fresh publication of even saved bytes
requires current admission/policy/lease. Response Frame2 is signed for that lease;
the immutable embedded P/A/K/Committed bytes retain exact retry semantics.

Bound bookkeeping to one active local intent and one accepted peer trigger per C,
plus the paid existing request/control outcomes. A second local request while one
is pending returns IntentAlreadyPending with the existing request ID. Same-ID/
same-body retry is exact; same-ID/different-scope/body refuses. Simultaneous A/B
requests bind their outcomes to the same saved P. Further notifications cannot
create an unbounded request log or consume C2 credit. The durable trigger survives
reopen; current permission is rechecked before doing anything with it.

Cancellation removes only the matching local pre-boundary intent and stops its
unsent/future notifications. It cannot unsend an in-flight/delivered OpenIntent,
delete A's already accepted peer trigger or roll back remote durable P. A's local
cancel does not cancel an independent peer trigger. B may therefore become
Prepared on a later valid P even after B's successful local cancellation, provided
current Consent2/scope/final gate still permit it. This is not global cancellation
or a new wire-cancel protocol. A first cancellation after local Offering/Prepared
returns TooLate. An exact retry of an earlier successful cancellation returns
that saved local result together with the actual current stage, never downgrades
the stage or claims the remote transition was cancelled.
Before handing a staged notification to transport, check the captured local
intent token again; cancellation before that handoff skips it. Once handed off,
delivery is potentially in flight and the local result says remote may proceed.
On A, actor/CAS order decides the race: cancel committed first with no independent
peer trigger makes a staged Propose stale; Propose committed first makes a first
cancel TooLate. A preserved peer trigger can still authorize the single Propose.

Upgrade status distinguishes LocalIntentPending, NotificationPending,
NotificationSentUnconfirmed, PeerIntentAccepted, ProposalDeferred,
CoalescedWithP(P_id), LocalIntentCancelledRemoteMayProceed, NoLocalIntent and
TooLate(current_stage). These are explicit local DTO outcomes, not new wire modes
or proof that the peer has committed. Status includes real durable stage,
local/peer trigger presence, saved control refs and policy/freeze reason.

## 4. Focused assertions and revised proposal

Extend the existing proposed C-s53-controls-01/C-s53-policy-01 groups:

1. A-only, B-only and simultaneous requests: exactly one A-signed durable P,
   exact coalesced outcomes, complete original refs and zero Native calls for
   intent/notification/P. Neither Hello/ModeView/Consent2 presence produces P.
2. Drop notification before A accepts it; reopen real B request state and repeat
   on a new binding. Then accept A's trigger, kill/reopen before P; it remains
   pending and creates one P only under a fresh current gate. Exact repeat and
   body conflict, policy rejection and changed-lease publication are distinct.
3. Cancel before send, cancel with an in-flight/delivered notification, A-local
   cancel with independent peer trigger, and B cancel before valid remote P:
   local notifications stop, but no false remote rollback/guaranteed cancellation.
   TooLate after P/Prepared preserves exact mode/native/debt.
4. Malformed/missing refs, incomplete phase, wrong role/origin/scope, expired or
   stale lease refuse before accepted trigger/state change. Test selected-stage
   duplicates without re-cut/reseal and bounded per-C bookkeeping.
5. Deterministic monotonic scheduler cases: first due,60s coalescing,45s whole/
   15s quiet windows, real protected L1 cycle versus failure/idle, no repeated
   admission starvation, C2 fairness and stale timer versus committed P/new lease.

In F-s53-control-loss-01, add a real public-command path on a small genuine L1
domain. With current Consent2/Locator2 at both peers and neither intent, observe
a bounded ALPN2 discovery ending, fresh genuine ALPN1 admission and committed L1
Text/Receipt progress. Then invoke only B's public `upgrade --ipc-file --slot
--request-id`; A.local_intent remains absent. Observe signed B OpenIntent on a
fresh existing lease, A's accepted trigger and saved P, then the existing lost
P/A/K/Committed/reconnect trace. Positive progress uses actual public commands,
packets and saved state, not injected intent flags or shortened test-only product
timers. Component drop/reopen tests above cover the notification-loss boundary;
the real P-loss/reopen path also checks B's remaining notification coalesces with
A's saved P. No exhausted checkpoint is needed for these additions.

Revise only the **proposed** F envelope from1100 to **1200**, and this F group's
work/whole from230/260 to **350/380**. The extra120s whole bound covers a real
discovery/protected-L1 scheduling opportunity and B-only command delivery; this
cannot be budgeted as free setup or erased with a clock jump. Use the previous
30s unassigned margin plus100s requested allocation; other four F bounds stay
140/420/180/70. New F whole-bound sum=1190, unassigned10, no automatic retry.
C360 and first C180/F0/P0 proposal are unchanged: added small component assertions
fit the existing groups; actual source/counts/readiness still need root clearance.

Closed charges remain C587.4486109/F4716.3201489/P0 including conservative C60.
If the revised full envelope is granted, cumulative maxima become
**C947.4486109/F5916.3201489/P0**; product-only C811.3567483/F5767.5783982/P0.
No prior allocation/remainder is reactivated, no budget is spent now, and S5.4/P
stays unassigned. Failure or a forecast not fitting these bounds returns to root;
it does not shorten an ordinary-L1 opportunity or retry silently.

## 5. Reopen checkpoint and stop

[Owner7::new](../../apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs#L160)
currently invokes InitializeHistory whenever both Ready controls exist. Make it
stage-aware within the already mapped owner/load implementation: incomplete L1
has no fake history; complete L1 initializes only a genuinely missing legal view
and otherwise validates/reuses it. Offering/Prepared and Selected/Active must
validate the already persisted HVI1 identity/cursor binding, never run L1-only
initialization over them. Missing/corrupt required state blocks. Reopen tests
record unchanged view/frozen suffix and zero gratuitous conversion/initialization.
This is an implementation checkpoint, not a new contract decision.

Submit this addendum for root acceptance. Original mapping, contract, historical
records and private dependencies remain unchanged. No product edit, build/test,
fixture traversal/open/restore, budget activation, staging or commit/push occurs
in this document-only revision.
