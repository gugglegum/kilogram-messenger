# RFC-0053: Opt-in runtime ticket automation

Status: M0.9.31 implemented (2026-09-05).

## 1. Goal

RFC-0051 and RFC-0052 made wide-area ticket exchange possible but required a
manual publish and refresh for every contact. M0.9.31 adds an explicit per-contact
policy owned by the one long-lived runtime actor. While that process is running,
it can renew the local ticket publication and fetch the peer publication before
their signed expiry.

This is addressing maintenance, not message storage, peer relay, first-contact
discovery, push notification, or an operating-system background service.

## 2. Explicit lifecycle boundary

Automation is disabled until the user installs an enabled policy for an already
enrolled contact through authenticated IPC v8. It runs only inside the existing
runtime process:

- closing the runtime stops all work;
- no Windows Task Scheduler entry, service, startup registration, or hidden
  daemon is created;
- enabling ticket automation does not volunteer the device as a relay and does
  not enable message storage for third parties;
- the existing out-of-band verified ticket remains necessary for first contact.

The Windows client exposes enable, disable, status refresh, and independent
Ethernet, Wi-Fi, mobile, and unknown-network permissions. Ethernet and Wi-Fi are
enabled in a new UI draft; mobile and unknown networks require explicit opt-in.

## 3. Authenticated policy state

Each policy is signed by the local Device key and binds:

- local Account and Device IDs;
- exact runtime contact, conversation and peer Account;
- canonical ticket-store base URL;
- enabled state, ticket TTL and refresh lead;
- retry base/max bounds;
- four network-class permissions;
- monotonic generation, previous policy ID, and configuration time.

Policy records are content-addressed append-only `Runtime` records in the
encrypted vault. Loading verifies the signature, exact content-addressed path,
contact binding, contiguous generation chain, monotonic time, and all numeric
bounds. Repeating byte-equivalent configuration reuses the current head;
changing or disabling it creates the next signed generation.

## 4. Scheduling and durable backoff

The runtime considers at most one ticket-automation action per scheduler check.
For every enabled and currently permitted policy it examines `publish` and
`refresh` independently. Each attempt creates a signed append-only head binding
the policy generation, action, attempt generation, previous attempt ID, result,
failure count, and next permitted time.

Successful work is scheduled for `signed expiry - refresh lead`. If a fetched
peer generation is already close to expiry, the next check is no sooner than 30
seconds to prevent a tight success loop. Failures use persisted exponential
backoff capped by the configured maximum. A restart therefore neither forgets
the failure count nor creates an immediate request storm. A new policy
generation deliberately starts a new retry scope.

Automatic publication creates the next signed publication generation. It does
not reseal an existing generation into different randomized HPKE bytes, because
the opaque store correctly treats different bytes at one generation as a 409
equivocation. Explicit manual publication keeps its idempotent replay behavior.

## 5. Runtime fairness

Polling is driven by a persistent monotonic interval. IPC activity no longer
recreates the timer, so frequent status reads from a GUI cannot starve message
delivery, automatic sync, or ticket maintenance. Delivery remains first,
automatic conversation sync second, and ticket maintenance third on each tick.
Network I/O runs without holding the state lock; signed result persistence
rechecks that the selected policy is still the exact current head.

## 6. Network policy

The runtime maps the platform provider to `ethernet`, `wifi`, `mobile`, or
`unknown` for every selection. A disallowed or unknown class performs no network
request and is reported as `network-blocked`. The policy stores no SSID, adapter
GUID, IP address, or network name.

The current Windows provider is an operational hint, not a security boundary: an
OS, driver, VPN, or tethering stack can classify a path imperfectly. The user can
always disable the policy or disallow a class.

## 7. IPC and observability

IPC v8 adds `ConfigureTicketAutomation` and `TicketAutomationStatus`. A typed
status reports policy generation, effective network class/permission, execution
scope, and separate publish/refresh state including last result, last success,
next attempt, consecutive failures, publication generation, and expiry.

The CLI exposes the same configure/status contract. Runtime logs contain contact
IDs, action, outcome and schedule but never ticket plaintext, HPKE body, bearer
token, seed phrase, SSID, or IP address.

## 8. Verification

The signed-state unit regression covers policy succession, tamper-resistant
encoding, bounded exponential retry, reset after success, and the minimum
near-expiry recheck. A real lifecycle test runs two live actors through the
production opaque store, enables all network classes only for the loopback
fixture, observes mutual automatic publish/refresh, disables one policy with a
new generation, and reloads the durable vault-primary state.

The Windows adapter regression checks the exact IPC v8 configuration, safe
defaults, typed status, and execution-scope claim. The test harness keeps Iroh
strictly on loopback, so Cargo's changing hashed test executable does not need a
Windows Firewall exception.

## 9. Honest limits and next work

The scheduler does not make the store available, anonymous, unlinkable, or
authenticated. Store operators still observe IP, channel, timing, size, and
declared generation. A party that learns a channel can still attempt a high-
generation availability attack.

Policy, attempt, publication, and observation histories are deliberately
append-only and currently protected by hard record-count bounds. Before a
long-lived public release they need an authenticated checkpoint/compaction
contract that preserves monotonic high-water evidence without unbounded startup
scans. The next protocol stage should also design an unlinkable write capability
or admission scheme for the opaque store; neither concern should be hidden by
calling the present result production-ready.
