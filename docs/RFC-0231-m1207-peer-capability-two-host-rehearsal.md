# RFC-0231: Two-host peer capability enforcement rehearsal

Status: proposed for M1.2.7; field evidence pending.

## Purpose

M1.2.6 moved the durable peer capability floor into the real outbound sync
path. This milestone requires the exact debug executable to exercise that path
between two physical Windows hosts through stable executable names.

The field harness first runs the established canonical sync v2 two-host
rehearsal. It then reuses only its disposable states to verify four properties:

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
