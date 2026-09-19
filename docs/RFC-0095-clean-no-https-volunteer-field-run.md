# RFC-0095: Clean volunteer delivery without an HTTPS mailbox fixture (M0.9.72)

Status: implemented; clean external two-host execution pending.

## 1. Purpose

M0.9.69 proved exact two-provider storage, recipient retrieval, deletion after
application commit, and restart safety. That harness still started a loopback
compatibility mailbox on Alice before sending. M0.9.71 changed the runtime so
an exact outbound item retires that HTTPS copy after two qualifying signed
volunteer receipts, but unit and static tests do not prove the complete real
network path.

M0.9.72 repeats the clean external topology with the HTTPS mailbox fixture
absent from the beginning. A successful run must show that Alice never attempts
the compatibility PUT, goes offline, and Bob still retrieves the same item from
both authenticated volunteer stores.

## 2. Compatibility descriptor

The current mailbox capability wire format still contains a compatibility
service URL and store public key. Removing that field is a later protocol
migration and is deliberately outside this evidence stage.

The M0.9.72 harness therefore uses:

- an unreachable numeric-loopback endpoint, `http://127.0.0.1:18787`;
- the valid public key from an RFC 8032 Ed25519 test vector as an inert expected
  store identity;
- no `kilogram-ticket-store.exe` artifact;
- no process that listens on the compatibility endpoint.

Those values cannot provide delivery. If the runtime attempts the old PUT, the
run fails instead of silently succeeding through a fixture.

## 3. Causal order

The established clean-run order remains unchanged:

1. Alice creates the run and proves the compatibility fixture is absent.
2. Two transport-distinct volunteer providers start before mailbox activation.
3. Bob imports exactly those offers, creates the Device-signed exact locator,
   and completes capability convergence with Alice using fresh runtime tickets.
4. Alice rechecks fixture absence, queues one unique message, resolves the
   committed providers and obtains two qualifying signed receipts.
5. The runtime atomically commits exact volunteer durability and emits
   `runtime_mailbox_http_put=not-attempted`.
6. Alice's runtime stops before Bob begins retrieval.
7. Bob polls the same two store keys, commits the message, deletes both replicas,
   restarts, and observes no redelivery.

The controlled field profile retains `route_policy=auto` with the previously
validated `aps1` relay fallback. This pins the experiment, not the production
relay policy, and still permits a direct-path upgrade.

## 4. Required sender evidence

The sender log must contain exactly one of each:

- `runtime_mailbox_https_compatibility_copy=suppressed-exact-volunteer-durability`;
- `runtime_mailbox_http_put=not-attempted`;
- `runtime_mailbox_delivery_durability=exact-volunteer-replication`;
- exact authenticated discovery and resolution `2/2`;
- two signed receipts from the precommitted store-key set;
- final `runtime_outbound_status=mailbox-stored`.

It must contain none of:

- `runtime_mailbox_http_put=attempted`;
- `runtime_mailbox_delivery_durability=https-compatibility`;
- a retained compatibility-copy reason;
- `runtime_mailbox_exact_completion_status=failed`;
- legacy random provider discovery.

## 5. Absence evidence

Closed evidence files are written before identity/capability preparation and
immediately before send. Both must state that:

- the compatibility store executable is absent from the kit;
- no compatibility fixture process was started;
- the exact inert endpoint is unreachable.

The post-send offline boundary repeats the absent binary, unreachable endpoint,
and unattempted PUT facts after Alice has stopped. This makes the claim stronger
than merely observing a connection refusal after an attempted upload.

## 6. Fail-closed verifier

The M0.9.72 verifier first runs the complete M0.9.69 exact-locator verifier with
the `m0972` label prefix. It then adds the no-HTTPS constraints above. Its
synthetic negative tests reject at least:

- an attempted HTTP PUT;
- a reachable compatibility endpoint.

The final evidence also retains the provider/commitment equality, fresh ticket,
Alice-offline, commit-before-delete, no-redelivery and exactly-once history
requirements from RFC-0092.

## 7. Packaging and operator flow

The generated Windows kit keeps the familiar three folders and six launches:

- `1`: Alice preparation, send and final verification;
- `2`: both volunteer providers;
- `3`: Bob preparation and receive/restart verification.

Only the stable debug `kilogram-cli.exe` is shipped. The generator uses two
Cargo jobs by default, creates no release build or ZIP, starts no network
process, and records SHA-256 plus byte length for every artifact.

## 8. Completion criterion

Implementation and a generated clean kit are not field completion. M0.9.72 is
closed only after one fresh two-host run produces evidence accepted by the
fail-closed verifier. A failed or interrupted attempt is not resumed as clean
evidence; a fresh kit/run is generated instead.
