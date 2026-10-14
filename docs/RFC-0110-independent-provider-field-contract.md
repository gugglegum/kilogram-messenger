# RFC-0110: Independent volunteer-provider field contract (M0.9.87)

Status: reduced two-host field run verified; strict independent-provider run pending.

## 1. Purpose and claim boundary

M0.9.86 can avoid a provider that is positively known to share a previously
observed local path domain with one already selected provider. It still cannot
turn unequal or absent local tags into evidence of separate machines, network
operators or failure domains. M0.9.87 therefore adds a controlled field
contract, not a new production trust primitive.

The experiment uses a minimum of three physical hosts and two provider
processes on different machines, access networks and declared operators. Four
hosts are ideal. The resulting evidence is controlled self-attestation and not
a protocol proof of operator, person, ASN, location or Sybil independence.

## 2. Four-folder topology

The synchronized kit contains four role folders:

1. `1` — Alice;
2. `2` — Provider 1;
3. `3` — Provider 2;
4. `4` — Bob.

With three hosts, Bob and Provider 1 may share one host. Provider 1 and
Provider 2 may not share a host. They must also use different access networks
and operator labels. With four hosts every role is separate.

Private account roots, device secrets, Redb state, IPC descriptors, provider
storage and open runtime logs remain below `%LOCALAPPDATA%`. Only atomic public
offers, bounded attestations and closed final logs enter the synchronized
folder. This prevents the earlier failure mode where a cloud-sync client held
or indefinitely uploaded a provider log that the runtime was still writing.

## 3. Provider publications

Each provider publishes three bounded files:

- one signed volunteer offer;
- one publication binding offer hash, expiry and attestation hash to the clean
  run, source revision and fixed provider role;
- one attestation containing only run-scoped SHA-256 values for machine,
  declared operator and declared access network.

The machine value is derived from Windows `MachineGuid`, but the raw value is
never serialized. Operator and network labels are normalized in memory and
only their domain-separated run-scoped digests are retained. The evidence
schema is closed: additional raw-label fields make verification fail.

Before mailbox activation, a helper validates both offer/publication pairs and
requires distinct machine, operator and network digests. Provider 1 is the
single writer for the aggregate manifest and ready marker; Provider 2 only
publishes its role-owned offer/publication/attestation and waits for that
marker. This avoids a multi-writer conflict when two synchronization clients
observe the shared directory at different times. Atomic replacements use a
bounded retry for transient reader/antivirus locks and still fail closed after
the bound. Missing or temporarily inconsistent cloud files keep the run
waiting; a positive equality fails closed.

## 4. Lifecycle and final evidence

The inherited service-free v2 lifecycle remains unchanged:

1. providers are online before Bob creates the exact capability;
2. Alice and Bob converge on one recipient-authenticated two-store commitment;
3. Alice obtains two signed volunteer receipts and stops;
4. Bob retrieves and commits both replicas, deletes after commit and restarts
   without redelivery;
5. Alice requests provider shutdown and verifies the complete evidence set.

Each provider first stops its runtime, then atomically copies closed final logs
to shared evidence. Both provider processes wait for the other's stop marker,
so delayed synchronization cannot leave final verification waiting forever on
a marker that no live process can create.

## 5. Verification

`verify-kilogram-m0987-independent-provider-evidence.ps1` inherits the exact
M0.9.69 lifecycle under the `m0987` label and additionally requires:

- service-free capability v2 and no compatibility endpoint;
- exact offer and attestation hash binding for both providers;
- two distinct run-scoped machine/operator/network claims;
- a closed attestation schema with no raw claim fields;
- sender offline before recipient retrieval, exact receipts `2/2`, two
  commit-before-delete results and no restart redelivery.

Its self-test rejects same-machine, same-operator, same-network and injected
raw-field fixtures. The static kit contract also checks four-folder placement,
local private state, publish-after-close ordering and stable debug-only build
policy.

## 6. Stage boundary

The generator uses Cargo `--locked`, debug profile and two jobs by default. It
creates no release build, ZIP, HTTPS mailbox service, background installation,
new executable target, field connection or external publication. M0.9.87 is
accepted locally only after static/self-test regression; the stronger field
claim remains pending until a fresh external three- or four-host run succeeds.

## 7. Two-host reduced profile

The generator may explicitly create a `two-host-reduced` operator harness for
Alice + Provider 1 on one Windows PC and Provider 2 + Bob on a second Windows
PC. It still requires two distinct run-scoped machine pseudonyms, but records
operator/network claims without requiring them to differ. Its verifier reports
`independent_provider_field_acceptance=false` and a separate
`verified-reduced-two-host` result. This profile exercises the complete
service-free delivery lifecycle; it cannot close the independent-provider
field claim described in sections 1-6.

## 8. Reduced field result

Clean run `20261014-080905`, generated from revision
`162836bb7a54180c3a00915749cf17fe238019e2`, completed the whole reduced
two-host lifecycle with the verifier already present in the kit. Alice obtained
two signed volunteer receipts and stopped before Bob retrieval; Bob committed
the message, deleted both replicas and observed no redelivery after restart.
The result is `verified-reduced-two-host` with
`independent_provider_field_acceptance=false`.

This is the current field-tested runtime baseline. It proves the service-free
two-replica lifecycle across the available PC and laptop, but it does not
replace the three-/four-host strict experiment required to claim separately
operated providers and independent access networks.

## 9. Refreshed reduced field result after M0.9.92

The runtime-journal change invalidated the section 8 baseline exactly as the
candidate continuity gate required. A fresh clean run `20261014-104506`, built
from revision `d33dd0950484665ee397d161e37ce74743ee8f93`, repeated the complete
two-host lifecycle after the Windows extended-length-path fix.

Both providers registered with the pinned APS1 relay before publication.
Alice received two authenticated volunteer receipts, stopped before Bob
retrieval, and used no central mailbox descriptor or HTTPS compatibility
store. Bob committed the message from both replicas, deleted both only after
commit, restarted, and observed no redelivery. The unchanged in-kit verifier
reported `verified-reduced-two-host` and
`independent_provider_field_acceptance=false`.

This run supersedes section 8 as the current field-tested runtime baseline. It
still does not claim separate operators or access networks and therefore does
not close strict independent-provider field acceptance.

## 10. Refreshed reduced field result after M0.9.96/M0.9.97

The task-aware test change and configured-relay diagnostic changed the coarse
runtime surface again. Fresh run `20261014-141058`, built from exact product
revision `be80d28ec12b41987bdbfa800016fb86b54b4723`, therefore replaces the
section 9 baseline.

Alice and Provider 1 ran on the desktop; Provider 2 and Bob ran on the
SSH-controlled Windows laptop. Every endpoint-bearing log proves the canonical
APS1 `relay_config_url`; absence of an observed home under `auto` is not
misreported as successful relay registration. Alice obtained exact 2-of-2
authenticated volunteer receipts and stopped before Bob retrieval. Bob
committed the message once, deleted both replicas after commit, and observed no
redelivery after restart. The strict verifier reports
`verified-reduced-two-host` and
`independent_provider_field_acceptance=false`.

This remains a reduced two-host service-free lifecycle result. It does not
prove separate provider operators or access networks and does not close the
strict independent-provider field claim.
