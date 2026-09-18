# RFC-0090: Three-folder M0.9.67 operator harness

Status: implemented and network-free verified; external run pending.

## Outcome

The verbose protocol-oriented M0.9.67 procedure is wrapped by a disposable
Windows field harness with exactly three operator folders:

- `1` is Alice plus a loopback-only compatibility mailbox fixture;
- `2` starts two independent volunteer identities on Alice's computer;
- `3` is Bob on the laptop.

The operator launches six numbered scripts. They create fresh test Accounts,
Device certificates, membership, runtime tickets, contacts and the Bob receive
mailbox automatically. Coordination uses paths relative to the kit and public
files under `1/shared`; the operator never copies IDs or edits a config.

## Local working state

Disposable Account Roots, keys and Redb state live under
`%LOCALAPPDATA%/Kilogram/M0967/<run-id>`. This is not a secrecy requirement for
these test identities. It prevents a synchronizer from copying a live database
while the runtime writes it. The shared Yandex directory contains public
tickets/offers, bounded evidence and completion markers.

## Compatibility boundary

Alice temporarily runs the existing `kilogram-ticket-store.exe` on numeric
loopback `127.0.0.1:8787`. Alice can therefore complete the still-required
compatibility upload without TLS or an external server. The fixture is stopped
with Alice before Bob retrieves both replicas over Iroh. Bob cannot reach
Alice's loopback store, so successful Bob evidence still requires both
volunteer sources.

This fixture is test scaffolding, not a production server or a claim that the
HTTPS compatibility path has been retired.

## Process and artifact boundary

Both providers and both clients execute the same stable-path
`kilogram-cli.exe`; the kit contains one copy. Generation builds debug binaries
with bounded Cargo jobs, creates no archive or release build and starts no
network process. `kilogram-ticket-store.exe` is the already-existing
loopback-only test fixture.

The final script stops both providers and invokes the unchanged fail-closed
M0.9.67 verifier. Thus operator simplification does not weaken the required two
store keys, two transport identities, two sender receipts, sender-offline
boundary, two Bob commits/deletes, restart and exact history checks.

## Interrupted-run recovery

File existence alone is not runtime readiness: launch scripts remove a stale
IPC descriptor and wait for a successful authenticated ping. A failure before
queue insertion archives its logs and may retry normally. If queue evidence,
the compatibility-store receipt and an incomplete durable volunteer plan all
exist, Alice resumes that exact plan without creating another message.

The field-discovered response-flush fix requires already running providers to
restart with the corrected stable-path executable. The optional provider
restart script reuses their existing private identities and stores, archives
the pre-fix transport logs, publishes fresh signed offers and leaves the same
final fail-closed evidence contract in force.
