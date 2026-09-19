# RFC-0099: Service-free v2 volunteer mailbox field run (M0.9.76)

Status: implemented and packaged locally; fresh two-network field execution pending.

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
Copying it to the synchronized two-host test directory and starting network
processes remain explicit later actions.
