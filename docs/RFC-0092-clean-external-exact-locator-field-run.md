# RFC-0092: Clean external exact-locator field run (M0.9.69)

Status: implemented and verified by one clean external two-host field run.

## 1. Purpose

M0.9.67 proved that two volunteer stores could retain, serve and delete an
offline message, but its mailbox capability had been created before the
volunteer offers were available. That run therefore exercised the bounded
legacy provider sample and could not validate RFC-0091.

M0.9.69 changes the causal order: providers exist before mailbox activation.
Bob imports one fresh, hash-consistent publication containing two signed,
transport-distinct offers. Only then does Bob create the mailbox offer and the
Device-signed exact replica-set commitment.

## 2. Operator shape

The generated directory contains three folders and six numbered launches:

1. `1/01_PREPARE_ALICE.ps1` creates the run and waits;
2. `2/01_START_PROVIDERS.ps1` starts both volunteer identities before mailbox
   activation and remains open;
3. `3/01_PREPARE_BOB.ps1` imports both offers, creates the exact-locator
   capability and keeps Bob online until Alice durably applies it and Bob
   receives the session-bound acknowledgement;
4. `1/02_SEND_ALICE.ps1` imports fresh offers, sends one message, obtains two
   store-signed receipts and stops Alice plus the loopback fixture;
5. `3/02_RECEIVE_BOB.ps1` resolves only the committed stores, commits and
   deletes both replicas, then proves restart does not redeliver them;
6. `1/03_VERIFY.ps1` stops providers and performs the final fail-closed check.

The two clients remain on different operator hosts. The two provider identities
may share Alice's host, so this run proves protocol, transport, locator and
storage mechanics, not operator independence or Sybil resistance.

The preparation scripts first exchange bootstrap tickets, then start the final
runtime processes. Because each runtime bind creates a fresh transport endpoint,
both sides wait for the peer's fresh runtime tickets to replace the bootstrap
files and validate those signed descriptors through the live runtime IPC before
capability convergence. A stale ticket or a descriptor for another Account or
Device fails closed instead of being mistaken for a network timeout.

## 3. Exact evidence contract

The verifier requires the same commitment ID in Bob's activation, Alice's
replication and Bob's retrieval. The two provider store keys must equal:

- Bob's pre-activation imported set;
- Alice's exact network-attempt and signed-receipt set;
- Bob's exact poll and committed-source set.

Both sender and receiver must report `exact-authenticated` and resolution
`2/2`. Any `legacy-random-fallback`, unrelated provider substitution, missing
capability acknowledgement, partial store set or repeated delivery fails the
run. A synthetic positive/negative self-test proves that legacy fallback, a
substituted poll key and reuse of a stale bootstrap endpoint ticket are rejected.

## 4. Synchronization and privacy

Yandex Disk carries only binaries, scripts, public tickets/offers, bounded logs
and test evidence. Live Redb databases and private state stay under
`%LOCALAPPDATA%\Kilogram\M0969`, avoiding synchronization of an open database.
Provider files are consumed only after one publication manifest, two hashes and
two expiry values form a consistent local snapshot.

Consumers derive the committed provider keys from the closed Bob
pre-activation import evidence. They never wait for provider runtime logs:
those logs remain open and change while the providers serve mailbox traffic,
so a file synchronizer is allowed to defer them until provider shutdown.

An interrupted runtime may leave Redb requiring its normal writable recovery.
If read-only mailbox-ledger inspection reports Redb `RepairAborted`, the runtime
performs exactly one writable recovery under the runtime state lock and the
vault dual-write guard, then retries read-only inspection. Other inspection
errors remain fail closed.

The controlled field harness pins every client and provider profile to the
previously field-proven `https://aps1-1.relay.n0.iroh.link./` relay while
retaining route policy `auto`. Direct-path upgrade therefore remains allowed;
only relay fallback selection is deterministic. This is a test-fixture choice,
not a production singleton, central-service dependency or product default.
The evidence verifier requires the exact route policy and relay URL in the
manifest and every retained runtime phase, and rejects a divergent relay.

The replica set remains inside the Device-signed capability exchanged by the
two contacts. Providers receive only requests for their own store. No global
directory, central mailbox service, additional executable or listener is
introduced.

## 5. Compatibility boundary

The test still performs an HTTPS compatibility upload to the existing loopback
store on Alice. It stops that fixture together with Alice before Bob starts
retrieval, so Bob's successful receive can only come from the two volunteer
Iroh stores. This does not yet authorize removal of the compatibility path.

Retiring that copy requires successful clean field evidence, an upgrade or
rotation policy for legacy capabilities, and a separately reviewed availability
rule for cases where too few current provider offers exist.

## 6. Build and resource boundary

The generator requires clean HEAD, runs the static gates first, and builds the
two existing stable-name executables in the debug profile with two Cargo jobs by
default. `BUILD-INFO.json` binds both executables, all six operator scripts,
their common helper, the evidence verifier and the precomputed boundary log by
SHA-256 and length; every numbered step checks that manifest before acting. It
creates no ZIP, performs no release build and starts no network process.
Network activity begins only when an operator explicitly runs the numbered
scripts.

## 7. Failed clean attempt and retry rule

The clean2 external attempt correctly formed identities, exact provider offers,
membership, fresh signed endpoint tickets and the `2/2` capability commitment.
Both convergence runtimes then selected the automatically assigned `euc1`
relay, and both directions repeatedly timed out before capability delivery.
The same environment had previously completed strict relay delivery and sync
through `aps1`, while `euc1` had already produced the same failure signature.
The run therefore failed at transport availability, not at Yandex Disk, Redb,
VPN hygiene or operator ordering.

The provider runner's open logs and publication manifest may remain in a
continuous synchronization state while it refreshes signed offers. They are
not causal input for consumers and are not a failure signal. After a failed
attempt the providers are stopped and a newly generated clean kit is required;
the old run is never resumed as clean evidence.

## 8. Verified clean field result

Run `20260919-144857`, built from revision
`6cea6074fbb4116639db3f79dddc0bb4bf373fe9`, completed on separate Alice and
Bob hosts/networks. Every retained runtime used `route_policy=auto` with the
pinned `aps1` relay. The fail-closed verifier reported `result=verified` and
proved all of the following:

- providers existed before capability activation and both peers completed the
  durable capability apply/acknowledgement handshake;
- sender and recipient resolved the same Device-signed commitment to exactly
  two providers, and Alice obtained two store-signed receipts;
- Alice runtime and the compatibility HTTP fixture were offline before Bob
  received two `volunteer-iroh` replicas;
- Bob committed each replica before its signed delete, restart produced no
  redelivery, and local history contained the message exactly once;
- no legacy random fallback or provider substitution occurred.

The final Alice, Bob and provider-stop markers were all present. This closes
the M0.9.69 clean external evidence requirement. It proves the implemented
two-provider delivery mechanics under the tested topology; it does not yet
prove provider operator independence, Sybil resistance or production relay
failover.
