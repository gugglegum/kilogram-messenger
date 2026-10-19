# RFC-0238: Canonical runtime IPC peer re-enrollment contract

Status: accepted for M1.2.14; dormant contract and negative corpus verified.

## Purpose and scope

M1.2.13 proved the complete peer re-enrollment flow with an exact binary on two
physical hosts. M1.2.14 defines the local UI/runtime boundary that can expose
that flow safely without yet connecting it to the active runtime actor or the
desktop application.

The contract lives in `kilogram-runtime-ipc`, but it is deliberately not a
variant of the active `RuntimeIpcCommand` or `RuntimeIpcResponse`. Runtime IPC
remains v31 and its descriptor signature remains v12. There is no adapter,
button, automatic operation, network activity, hot apply or public-policy
change in this stage. The public default remains `legacy-only`.

## Explicit multi-stage ceremony

The request vocabulary keeps five operations distinct:

1. `inspect` asks the runtime to verify an already staged invitation reference;
2. `plan` binds the exact invitation and durable predecessor quarantine;
3. `approve` requires the exact plan ID, the displayed `REENROLL` confirmation
   phrase and an explicit user-confirmed authorization value;
4. `accept` binds the exact signed tombstone and predecessor quarantine;
5. `status` reads the durable result without granting mutation authority.

Responses are restricted to the stage that was requested. An inspection
cannot claim acceptance, and a planning request cannot skip explicit approval.
The contract has no combined toggle, implicit consent, automatic quarantine
clear or hot runtime activation. A committed acceptance explicitly reports
`accepted-restart-required`; only a later fresh runtime start may consume the
prepared transition under the M1.2.12 rules.

## Privacy-minimizing projection

The UI projection contains only an opaque contact ID, verified invitation
summary, exact artifact IDs, bounded status values and the confirmation phrase.
It does not expose raw Account IDs, raw Device IDs, Root or Device secrets,
ticket or endpoint data, queue IDs, message bodies or capability bytes.

The replacement Device is represented by a domain-specific fingerprint rather
than its raw identity key. `publication_epoch` is intentionally neutral UI
terminology; the ticket artifact itself remains below the runtime boundary.
Runtime-owned invitation staging is outside this codec and must be designed
before an active adapter is permitted.

## Canonical request and signed response

Requests and signed responses use bounded Postcard encoding and must re-encode
byte-for-byte. A request is bound to the exact authenticated runtime descriptor,
including its ephemeral instance, so replay against a replacement runtime is
rejected before adapter dispatch.

Every response is signed by the current Device named by that verified runtime
descriptor. The signed content exact-binds a domain-separated request digest,
the exact descriptor digest, a domain-separated current-Device binding, the
opaque contact, the predecessor quarantine summary, the verified successor
summary and the durable outcome. The wire omits the raw signer Device ID;
verification uses the already authenticated descriptor.

Changing the request ID, command, descriptor instance, current Device,
response state or durable artifact invalidates verification. The maximum
canonical request is 4 KiB and the maximum signed response is 16 KiB.

## Status and authority semantics

The closed status vocabulary is:

- `missing-invitation`;
- `unsafe-or-divergent` with a bounded hazard reason;
- `ready`;
- `approval-required`;
- `approved-prepared`;
- `accepted-restart-required`;
- `consumed`.

Durable predecessor quarantine and process-local scheduler suppression are
separate fields with consistency checks. A paused in-memory scheduler never
becomes evidence of durable authority, and retained quarantine is never
reported as cleared merely because a successor transition was accepted or
consumed.

## Old history remains a separate authority boundary

Peer re-enrollment restores future communication with an authorized successor;
it does not give the replacement Device old pairwise ratchet keys. The response
therefore carries an independent history-recovery state. Without a separately
approved same-account history rewrap source it says
`separate-approval-required`, not “history recovered.” A completed rewrap must
name an exact non-empty source inventory. This contract cannot silently infer
complete history from successful peer re-enrollment.

## Verification and non-claims

Eight stable-path Rust tests cover all five request stages, canonical
round-trips, explicit approval, confirmation syntax, descriptor substitution,
request and current-Device binding, stage skips, inconsistent durable claims,
process-local suppression, independent history recovery and privacy-safe public
projection. The boundary verifier adds eight negative mutations and preserves
the absence of a new active command variant. Aggregate readiness contains 140
closed gates.

M1.2.14 does not provide an active runtime IPC adapter, desktop controls,
runtime-owned invitation import, automatic recovery, quarantine deletion, hot
apply, public rollout authorization, a release artifact or an independent
security review. No network or second host is required for this contract-only
stage.
