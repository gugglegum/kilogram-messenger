# RFC-0237: Two-host fresh-start peer re-enrollment rehearsal

Status: accepted for M1.2.13; exact-revision two-host evidence verified.

## Purpose

M1.2.10 through M1.2.12 separated peer recovery into an offline plan, a
durable acceptance receipt and a fresh-start runtime transition. M1.2.13
requires those boundaries to work together with the ordinary queue, ratchet,
transport and sync paths on two physical Windows hosts using the exact same
stable-name debug executable.

The rehearsal starts with an authenticated peer capability record that has
already advanced to durable quarantine. Alice enrolls that predecessor as the
runtime contact and queues one sealed message. A runtime attempt proves the
quarantine blocks the item before endpoint construction or body
materialization. Bob then creates a different Device under the same Account
Root and publishes a current invitation.

The old Bob Device has readable local projections for six existing messages,
while the replacement Device does not possess their old pairwise ratchet keys.
Those projections are therefore transferred by the existing authenticated
same-account history-rewrap mechanism before network sync. Re-enrollment does
not silently weaken E2EE by making a replacement Device decrypt ciphertext for
which it was never a recipient.

Alice performs the explicit plan, approval and acceptance ceremony entirely
offline. The resulting transition is prepared but unconsumed. Replacing the
accepted successor descriptor with the predecessor ticket makes the next
runtime start fail closed with zero peer network attempt. Restoring the exact
descriptor lets a fresh runtime consume the successor before construction of
the Iroh endpoint. The ordinary scheduler then delivers the pre-existing
sealed queue item once over direct LAN and records its acknowledgement.

A second Alice runtime start sees the exact transition as already consumed,
performs zero outbound actions and does not duplicate delivery. A final sync
converges eight events on the replacement Bob Device. Both histories contain
the queued message exactly once. The predecessor contact file remains
byte-exact and the old capability quarantine remains durable throughout.

## Evidence boundary

The local and remote executable SHA-256 values must match. Both product data
sessions use Iroh direct LAN; SSH is only the control and artifact-transfer
channel. The harness uses the stable paths
`.tmp/kilogram-field-runner/kilogram-cli.exe` and
`KilogramLab/field-kit-current/kilogram-cli.exe`, so Windows Firewall does not
see a new hash-derived executable name for every run.

Raw logs, state directories, Account and Device IDs, tickets, endpoint
addresses, event and queue IDs, message bodies and secrets remain in the
ignored field directory. The tracked evidence contains only the exact source
revision and binary hash/size, bounded counts, topology labels and semantic
outcomes.

This rehearsal does not change the connection-ticket wire, sync wire, runtime
IPC or public policy. The public default remains `legacy-only`. It proves one
direct-LAN replacement flow, not automatic peer re-enrollment, not a desktop
recovery ceremony, not relay portability and not an independent security
review. Generic capability stripping on an unknown first contact remains
indistinguishable without prior authenticated state.

## Accepted evidence

Run `m1213-field-20261019T171026Z` completed on implementation revision
`d307e965f338e11ee786af75a8e3f71309b42fa3`. Both physical Windows hosts used
the same 62,375,424-byte debug `kilogram-cli.exe`, SHA-256
`31e3b6e1a925526b5fee1d1bbe4205eb3ed732106e5b1ca479c3e5c5855d4f0f`.

The first diagnostic run reached final sync and correctly rejected the six
old events on the replacement Device: they were authored by the predecessor
for Alice and the replacement had neither a recipient slot nor a local
projection. The harness was corrected to perform explicit offline history
rewrap rather than weaken recipient-account validation. The accepted fresh run
then rewrapped all six old messages, rejected descriptor substitution before
networking, consumed the exact successor before endpoint construction,
delivered and acknowledged the queued message exactly once, survived an
idempotent restart and converged all eight events with equal histories.

The bounded tracked record is
`CANONICAL-SYNC-V2-PEER-REENROLLMENT-TWO-HOST-EVIDENCE.json`. No release build,
ZIP/archive, Git tag or automatic GitHub build was created.
