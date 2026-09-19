# RFC-0099: Service-free v2 volunteer mailbox field run (M0.9.76)

Status: implemented and externally verified by fresh two-network run.

## 1. Goal

M0.9.74 proved the complete volunteer mailbox lifecycle while the v1
capability still carried an inert HTTPS tuple. M0.9.75 introduced the
service-free `v2-exact-volunteer` wire format. M0.9.76 must prove that same
lifecycle using a capability that never contains a mailbox URL, central store
key or `MailboxServiceDescriptor`.

This is a fresh evidence run, not a continuation or relabelling of M0.9.74.
It uses a distinct `M0.9.76` build milestone, `m0976` conversation/message
prefix and `%LOCALAPPDATA%\Kilogram\M0976` private-state root.

## 2. Topology and operator flow

The existing three-folder, six-launch topology is retained:

- folder `1`: Alice preparation, send and final verification on the desktop;
- folder `2`: two independent volunteer provider processes on the desktop;
- folder `3`: Bob preparation and receive/restart test on the laptop;
- `1\shared`: public coordination and bounded evidence synchronized by the
  user's existing file-sync service;
- live identities, secrets and Redb state stay outside the synchronized kit
  under `%LOCALAPPDATA%`.

The provider runtimes must be online and their fresh offers imported before
Bob creates the mailbox capability. Alice and Bob use fresh enrolled Devices,
fresh runtime tickets and the pinned `aps1` relay in `auto` route mode; a direct
upgrade remains allowed.

## 3. Service-free activation boundary

Bob invokes `runtime-mailbox-exact-offer-create`. The command has no
`--service-base-url` or `--store-key` argument. Both Bob's activation output
and Alice's recipient import must report:

- `mailbox_capability_format=v2-exact-volunteer`;
- `mailbox_service_descriptor=absent`;
- one Device-signed exact commitment containing two transport-distinct
  volunteer store keys.

Neither log may contain `mailbox_service_url` or `mailbox_store_key`. The kit
does not contain `kilogram-ticket-store.exe`, declares no compatibility
endpoint and starts no HTTPS fixture.

## 4. Delivery and recovery evidence

Alice queues exactly one marked message. Acceptance requires:

1. exact authenticated resolution of the two committed providers;
2. two store-signed, transport-distinct durable receipts;
3. `runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer`;
4. `runtime_mailbox_http_put=not-attempted` and no HTTPS durability line;
5. Alice's runtime stopped before Bob starts retrieval;
6. Bob commits and deletes both opaque replicas after application commit;
7. a Bob restart produces no replica redelivery;
8. Bob history contains the marked message exactly once.

An incomplete exact threshold remains pending and must not attempt HTTPS.
Reverse acknowledgements use the same service-free exact locator, although
this one-way field assertion does not require Alice to come back online to
consume one.

## 5. Evidence and generation safety

`verify-kilogram-m0976-service-free-v2-evidence.ps1` first applies the inherited
exact-locator verifier, then rejects any central tuple, compatibility endpoint,
legacy retained/suppressed copy or attempted HTTP PUT. Its negative self-tests
prove those failures are fail closed.

The generator:

- requires a clean exact Git revision;
- builds only the stable debug `kilogram-cli.exe` with at most two Cargo jobs;
- records SHA-256 and byte length for every shipped artifact;
- creates no ZIP or release build;
- starts no network process;
- adds no executable or server.

Generating a kit is not field completion. The milestone becomes externally
verified only after a fresh two-network run passes the final evidence verifier.

## 6. Prepared clean kit

The clean source revision `c39f5ce9ec6effd49cf355036119e28d830c9d28`
produced `.tmp\m0976-service-free-v2-c39f5ce9ec6e`. Independent local checks
verified all 11 manifest artifacts, all nine shipped PowerShell files and the
bundled negative verifier self-test. The 13-file kit is 57,863,721 bytes and
contains neither a ZIP archive nor `kilogram-ticket-store.exe`.

This local path is intentionally not a release artifact or external evidence.
Its 13 files were copied byte-identically to
`C:\Users\Paul\YandexDisk\!M\M0.9.76`; the field run must wait for complete
second-host synchronization. Kit generation and copying themselves started no
network process.

## 7. Rejected first attempt and harness correction

The first external attempt reached successful Alice/Bob capability convergence
but stopped before Alice runtime start, queue insertion or message creation.
`02_SEND_ALICE.ps1` incorrectly sent both legacy M0.9.69 and service-free
M0.9.76 through the `else` branch that starts `kilogram-ticket-store.exe`.
Because the v2 kit intentionally has no such executable, PowerShell rejected an
empty process argument before any delivery evidence was created.

The correction starts the compatibility process only under the explicit
`if (-not $noHttpsCompatibility)` guard. The boundary verifier now walks the
PowerShell AST and requires the sole store-start command to be structurally
owned by that exact guard. The interrupted run is rejected rather than resumed;
a new committed `M0.9.76-retry1` kit and fresh private state are required.

Correction revision `ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae` produced
`.tmp\m0976-service-free-v2-ff38e1b89dc5`. All 11 manifest artifacts,
the bundled negative verifier and the generated AST store guard passed. Its
13 files (57,863,913 bytes) were copied byte-identically to
`C:\Users\Paul\YandexDisk\!M\M0.9.76-retry1`; this is the only kit accepted
for the next attempt.

## 8. Accepted external result

Fresh retry run `20260920-001614` from exact revision
`ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae` passed the bundled fail-closed
verifier with `result=verified`. The retained evidence proves:

- capability format `v2-exact-volunteer` with no central service descriptor or
  compatibility endpoint;
- exact authenticated provider resolution and signed receipts `2/2`;
- `runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer` and
  `runtime_mailbox_http_put=not-attempted`;
- Alice runtime offline after durable replication and before Bob retrieval;
- two volunteer-Iroh inbound applications, each deleted only after commit;
- zero volunteer redeliveries after Bob restart;
- the marked message occurs exactly once in Bob history (`event_count=2`
  including its protocol companion event).

This accepts the M0.9.76 mechanism and two-host lifecycle boundary. It does not
prove physical/operator independence of the two providers, Sybil resistance or
access-correlation privacy because both provider processes still ran on Alice's
host for this controlled test.
