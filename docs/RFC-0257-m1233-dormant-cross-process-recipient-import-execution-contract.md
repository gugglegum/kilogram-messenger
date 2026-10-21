# RFC-0257: dormant cross-process recipient import execution contract

Status: accepted for M1.2.33; dormant contract and negative corpus verified.

## Purpose

M1.2.32 can execute one durable recipient-signed import intent, but only inside
one trusted process with caller-supplied live memberships. M1.2.33 defines the
bounded cross-process vocabulary needed to expose that capability later without
making a durable database the sole membership authority and without silently
turning an active intent into permission to import.

This is not a variant of the active `RuntimeIpcCommand`. Runtime IPC remains
version 33 with descriptor signature domain v14. No server dispatch, CLI,
desktop control, worker, timer or network path is registered in this stage.

## Explicit ceremony

The request vocabulary keeps five operations distinct:

1. `Plan` asks for a read-only execution plan over one exact intent,
   coordinator head, bounded step count and ordered membership references.
2. `Status` observes one intent and, optionally, one exact execution plan.
3. `Confirm` repeats the exact coordinator and membership bindings, carries a
   fresh nonce, an exact `IMPORT xxxx xxxx xxxx xxxx` phrase and explicit user
   approval.
4. `ApplyNext` names the confirmed plan and a separate mutation nonce and can
   return at most one step.
5. `ApplyBounded` does the same with an explicit hard-bounded limit of at most
   eight steps.

An active intent alone is never execution authority. Planning, confirmation
and execution results are separate signed stages. A stage-skipping response,
implicit confirmation, substituted plan, reused confirmation as mutation nonce
or result exceeding the requested budget fails closed. Execution never consumes
the M1.2.31 intent; its separate recipient-signed consume ceremony remains.

## Membership authority

The wire carries at most eight strictly ordered, unique references. Each entry
contains a caller-authorized context ID, authority reference ID and membership
digest, all opaque and content-addressed. Their canonical ordered digest binds
the plan, confirmation and later execution.

The references are not membership snapshots and grant no authority by
themselves. A future adapter must reload every referenced membership from the
caller-authorized live context immediately before planning or mutation and
exact-match the M1.2.32 candidate. A single durable recovery database cannot
substitute for this reload, and the contract performs no source discovery.

## Replay and response authentication

Every request is canonical Postcard, bounded to 8 KiB, carries a nonzero request
ID and is bound to the exact verified runtime descriptor. Mutation requests add
a distinct mutation nonce and the exact confirmation ID. A future mutation
adapter must durably deduplicate that pair before invoking M1.2.32; this dormant
contract deliberately does not claim such persistence yet.

Responses are canonical Postcard bounded to 16 KiB, bind the exact request
digest, runtime descriptor and current descriptor Device, and are signed by
that Device under a dedicated domain. Reordered, duplicate, stale, substituted,
partially decoded or non-canonical input is rejected before future adapter
dispatch. Every response states that membership reload and explicit execution
remain required and that the intent was not consumed.

## Privacy and authority boundary

The public projection contains opaque IDs, closed states, bounded counts and
aggregate import receipts only. It does not expose raw Account IDs, raw Device
IDs, membership snapshots, filesystem paths, tickets, endpoints, keys,
ciphertext history artifacts, message bodies or plaintext.

The contract grants no network, background, import, membership or consume
authority. It changes neither connection-ticket, sync nor history-transfer
wire formats. The public default remains `legacy-only`.

## Verification

Eight focused stable-path Rust tests cover all request stages, canonical
round-trip, strict membership order and bounds, explicit confirmation,
descriptor/request/device response binding, stage skipping, binding
substitution, exact mutation nonce and separate non-consuming confirmation.
The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 159 closed gates with rustfmt and locked,
offline, workspace-wide strict Clippy.

No product network, second host, release build, archive, tag or automatic
GitHub build is used. M1.2.34 may add only a dormant read-only adapter for
`Plan` and `Status`; mutation dispatch remains a separate later stage.
