# RFC-0094: Exact mailbox HTTPS-copy retirement (M0.9.71)

Status: implemented.

## 1. Outcome

An outbound conversation item addressed through an authenticated exact
replica-set capability now tries the committed volunteer stores before the
HTTPS compatibility store. After two transport-distinct signed volunteer
receipts have been verified and durably bound to that exact set, the sender
does not call HTTPS for that item.

This is a per-item retirement boundary. It does not remove the HTTPS protocol,
service descriptor or compatibility implementation from Kilogram.

## 2. Suppression conditions

The HTTPS copy is suppressed only when every condition below is true:

- the current peer capability contains a Device-signed exact replica-set
  commitment;
- the durable per-item replication plan is bound to the signed mailbox
  dispatch;
- the persisted locator commitment and canonical store-key set exactly match
  the authenticated capability;
- at least the required two transport-distinct signed volunteer receipts match
  the same encrypted request and come from keys inside that exact set;
- the client ledger atomically replaces the pending HTTPS upload with a durable
  exact-volunteer completion record.

Receipts retained from an earlier legacy random selection do not count after an
exact locator is installed. A store key outside the locator, a repeated
transport identity, a mismatched request, or an incomplete receipt threshold
therefore cannot suppress HTTPS.

## 3. Compatibility cases retained

The sender still attempts the HTTPS compatibility path for:

- a legacy capability with no authenticated exact locator;
- incomplete exact replication, including temporarily unavailable volunteer
  providers;
- a reverse acknowledgement, whose current upload record has no per-dispatch
  exact replication plan.

If HTTPS succeeds after incomplete exact replication, its copy remains valid
until normal recipient deletion or expiry. This milestone does not delete a
compatibility copy retroactively when volunteer replication later reaches its
threshold. That avoids inventing a cross-store transaction or claiming a
delete authorization that the sender does not have.

## 4. Durable state and restart behavior

The existing stored-outbound Redb table accepts a second framed record type for
exact volunteer completion. The old HTTPS receipt encoding remains unchanged
and byte-compatible. The new record retains the request digest, compatibility
service key, dispatch binding, exact commitment, canonical store keys,
transport identities and signed receipts.

Insertion of that record and removal of the pending upload occur in one
immediate-durability Redb transaction. A crash before it leaves the pending
request intact; on restart the already durable replication receipts are
revalidated and the transition is retried without another message or HTTPS
request. A crash after it cannot resurrect the pending compatibility upload.

## 5. Observability

Runtime evidence distinguishes:

- `suppressed-exact-volunteer-durability` with
  `runtime_mailbox_http_put=not-attempted`;
- `retained-legacy-capability`;
- `retained-incomplete-exact-replication`;
- `retained-compatibility-only-payload` for the reverse acknowledgement path.

The final durability source is emitted as either
`exact-volunteer-replication` or `https-compatibility`. Queue state remains the
existing `mailbox-stored` state, so this stage requires no IPC schema change.

## 6. Scope and non-claims

This change adds no server, no new executable and no background scheduler.
It does not prove that two providers have different operators or physical
failure domains, solve Sybil admission, hide access correlation, or retire
HTTPS for old clients. Reverse acknowledgements and non-converged capability
states deliberately preserve compatibility.

A clean external run with the HTTPS fixture absent from the beginning is the
next evidence stage. Until then, the unit and static gates prove the local
ordering and fail-closed transition but not a new real-network topology.

## 7. Verification

The mailbox-client regression proves that two exact receipts from distinct
transport identities atomically replace a pending upload, survive replay and
expire normally; duplicated transport identity is rejected. The replication
test proves that a legacy receipt outside the exact locator is not qualifying.
The CLI policy test covers legacy, incomplete exact and satisfied exact cases.

`scripts/verify-kilogram-mailbox-https-retirement-boundary.ps1` additionally
checks call ordering, exact commitment equality, durable transition ordering,
legacy decoder compatibility, retained fallback cases and the no-new-EXE
boundary.
