# RFC-0231: Two-host peer capability enforcement rehearsal

Status: accepted for M1.2.7; exact-revision two-host evidence verified.

## Purpose

M1.2.6 moved the durable peer capability floor into the real outbound sync
path. This milestone requires the exact debug executable to exercise that path
between two physical Windows hosts through stable executable names.

The field harness first runs the three established basic canonical sync v2
two-host scenarios to prepare authenticated disposable pairs. It deliberately
does not replay the historical M1.1.84 containment expectation: after M1.2.6,
a fresh conversation cannot distinguish intentional capability suppression
from stripping once the peer capability floor was pinned. It then reuses the
three disposable pair states to verify four properties:

1. an extension-free first-contact ticket completes legacy sync without
   creating a capability store;
2. an authenticated canonical candidate has already produced durable sequence
   one before its legacy transport session completes;
3. removing that candidate later produces durable sequence-two quarantine and
   no `transport_endpoint_id`, even while no listener is present;
4. a mutually signed selected canonical session resumes through a fresh
   extension-free ticket using persisted-selection evidence and never falls
   back to legacy.

## Evidence boundary

The local and remote executable SHA-256 values must match. SSH is only the
control and artifact-transfer channel; the product data path is Iroh direct LAN
between the two physical hosts. Raw logs, states, ticket bodies, Account and
Device IDs, endpoint addresses, selection evidence IDs and secrets remain in
the ignored field directory. The eventual tracked record is bounded to source
revision, executable hash/size, semantic outcomes and topology labels.

This rehearsal does not change a wire format, runtime IPC or the public
activation policy. The public default remains `legacy-only`. It is portability
and integration evidence, not an independent security review and not proof of
generic first-contact stripping detection.

## Accepted evidence

Run `m1207-capability-20261019T101809Z` completed on source revision
`120fa77ff7e832fb9a285f72dfd377ee39825296`. Both physical Windows hosts used
the same 61,414,912-byte debug `kilogram-cli.exe`, SHA-256
`1b58772af5ebbee4c46c929f78b2a822fc39c9dcfef47a3f50dbf0d3fb2c300b`, through
stable executable paths. The product path was direct Iroh LAN; SSH was only
the control and artifact-transfer channel.

The accepted run prepared all three base pairs and passed all three field
scenarios. The legacy first contact converged six events without creating the
store. The candidate was sequence one before stripping and quarantine became
sequence two without endpoint creation. The selected canonical resume kept
sequence one, reused the exact persisted evidence on both roles, converged six
events on `/cbor/2` and reported no fallback.

The first field attempt exposed a now-obsolete historical harness expectation:
post-selection containment suppressed the current capability and a new
conversation was correctly quarantined by the M1.2.6 floor. Pair preparation
was narrowed to the three relevant current scenarios rather than weakening
the product check. The second attempt exposed a harness-only assumption that a
ticket file contained plain JSON; tickets are base64url JSON envelopes. The
accepted run used an explicit decode/remove/encode round-trip and therefore
reached the product's real signature and capability checks.

The bounded tracked record is
`CANONICAL-SYNC-V2-PEER-CAPABILITY-TWO-HOST-EVIDENCE.json`. Raw logs, state,
ticket bodies, endpoint addresses, selection IDs, Account/Device identifiers
and secrets remain only in the ignored field directory.
