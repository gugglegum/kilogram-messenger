# RFC-0063: Roster-wide own-device availability automation (M0.9.41)

Status: implemented in M0.9.41.

## 1. Problem

M0.9.40 removes the shared ticket-file dependency between two already
authorized Devices, but an operator still has to configure a separate policy
for every ordered Device pair. That does not scale to a normal multi-device
Account and is easy to leave partially configured after a Root roster change.

This slice introduces one local Device-signed policy that projects the exact
current Root-signed Account roster into the existing pairwise discovery and
announcement schedules. It automates availability exchange between already
authorized Devices. It does not enroll a new Device, distribute its prekeys or
turn the opaque ticket store into Account authority.

## 2. Signed roster policy

Every runtime Device keeps one append-only `SignedOwnDeviceRosterPolicy`
chain. A generation binds:

- the local Account and Device IDs;
- the exact Root authority revision and digest of the canonical Device list;
- enabled state and canonical ticket-store URL;
- locator TTL and refresh lead;
- announcement interval and envelope validity;
- bounded exponential retry values;
- independent Ethernet, Wi-Fi, mobile and unknown-network permissions.

The local Device signs every generation. Equal configuration against the same
roster is idempotent. A different Root roster is a configuration change even
when every operator setting is unchanged, so reconciliation leaves an
authenticated transition in the chain.

The policy is local Device state, not an Account Root instruction. Every
running Device that should participate configures its own policy. This avoids
giving the GUI, store or another Device authority to mutate local network and
metering preferences.

## 3. Deterministic reconciliation

For every other Device in the current exact roster, reconciliation creates or
updates both existing child chains:

1. a directional pairwise ticket-discovery policy;
2. its authenticated endpoint-announcement schedule.

Child configuration is derived only from the signed parent policy. Ticket
paths are deterministic under the runtime-managed public discovery directory.
The union of active recipients and historical child recipients is processed in
Device-ID order and remains bounded by the Account device limit.

When a Device disappears through a valid permanent Root revocation, its child
policies receive a new disabled generation. Historical records and attempts
remain auditable, but the scheduler performs no more network work for that
recipient. Other active recipients receive a generation bound to the new
roster because the pairwise locator capability itself rotates with the roster
digest.

When a newly enrolled Device is present in the complete authenticated runtime
ticket, reconciliation creates its two child chains automatically. The
existing live `ApplyOwnDeviceDirectory` command remains deliberately
removal-only: hot enrollment would also need the new Device's fresh signed
prekey pool and safe launch-profile convergence. Addition is therefore picked
up when the runtime starts from the updated complete profile/ticket; revocation
is reconciled immediately by the live removal path. This RFC does not weaken
the M0.9.27 enrollment boundary.

Reconciliation runs:

- when the roster policy is configured;
- immediately after a successful live Root-signed removal;
- when the runtime starts with a public own-device discovery directory.

The parent and all changed child records commit in one existing vault-primary
state transaction. A no-op restart or repeated configuration writes nothing.

## 4. Foreground scheduling and storm bound

The network workflow is unchanged: publish this Device's recipient-HPKE
locator, fetch and verify the sibling locator, then push the signed encrypted
endpoint/high-water bundle over an authenticated same-Account Device session.

The runtime actor selects at most one due child in deterministic Device-ID
order per five-second automation check. The complete publish/fetch/push
workflow finishes before another child is selected, and it shares the existing
global outbound-action limit. Per-recipient signed success/backoff state and
network permissions remain authoritative. Thus adding many Devices cannot
create parallel fan-out inside one runtime process.

This is a local hard bound, not a distributed mutex between machines. Several
authorized Devices can be online and independently run one workflow at the
same time. Avoiding every cross-device coincidence without a coordinator or a
clock/lease assumption is a separate protocol problem; the status API states
the execution scope as one serialized workflow while this runtime process is
running.

Foreground shutdown still stops all work. This slice does not install Windows
Task Scheduler entries, an OS service, autostart or a hidden background agent.

## 5. IPC and operator surface

IPC v15 adds:

```text
ConfigureOwnDeviceRosterAutomation { ... }
OwnDeviceRosterAutomationStatus
```

The CLI adapter exposes the same actor-owned operations:

```powershell
.\kilogram-cli.exe runtime-ipc-configure-own-device-roster `
  --ipc-file .\runtime.ipc.json `
  --service-base-url https://store.example/

.\kilogram-cli.exe runtime-ipc-own-device-roster-status `
  --ipc-file .\runtime.ipc.json
```

Ethernet and Wi-Fi are allowed by default. Mobile and unknown networks remain
denied unless explicitly enabled. Status returns the parent generation and
roster revision, active/configured/retired recipient counts, the current
network decision, `max_parallel_workflows=1`, foreground execution scope and
the existing detailed child status for every known recipient.

Once a roster policy exists, the older per-recipient configuration commands
fail closed. Mixing two independent configuration authorities would otherwise
let a manual child silently contradict the signed roster projection. The
global policy can be disabled while retaining its settings and audit history.

## 6. Persistence, compaction and compatibility

Roster policy records live in the encrypted vault-primary runtime snapshot,
have a separate 1024-record defensive bound and join the existing 4096-record
global runtime-ticket bound. After eight retained generations, transactional
checkpoint compaction authenticates the roster head and removes only the
verified prefix. The new checkpoint enum variant is appended, preserving old
postcard discriminants.

Connection ticket v10, pairwise locator publication, endpoint-announcement
envelope, ratchet/event formats and wire ALPN v8 do not change. Runtime IPC
advances from v14 to v15, so runtime, CLI adapter and desktop must be upgraded
together. Existing per-recipient policy records remain readable and are
adopted as children when the roster policy is first configured.

## 7. Verification

Regression covers:

- signed, bounded and append-only roster policy encoding;
- idempotent configuration against an unchanged roster;
- automatic child creation for all other active Devices;
- automatic creation after a complete expanded runtime roster;
- immediate disabling after live Root revocation;
- no-op reconciliation after restart on the same roster;
- typed IPC status before and after live removal;
- restart recovery from the authenticated removal receipt;
- checkpoint compaction and authenticated roster-head recovery.

## 8. Remaining work

M0.9.42 should converge accepted sibling publication evidence automatically
and detect conflicting same-generation observations across Devices. Blind
mailbox delivery, privacy-preserving gossip, cross-machine scheduling leases,
OS background execution and device enrollment remain separate work.

