# RFC-0045: Desktop device-link and multi-source recovery wizard

Status: implemented in M0.9.18 (2026-09-04).

## 1. Scope

M0.9.18 exposes the existing M0.9.17 enrollment ceremony and the existing
recipient-bound history recovery protocol through `kilogram-windows`. It does
not change authority, wire, encryption, recovery-plan, checkpoint, or
reconciliation formats.

The desktop remains an orchestration and presentation layer. Account Root,
device signing/encryption keys, ratchet state, vault contents, events, and
plaintext projections are not loaded by the GUI.

## 2. Device-link ceremony

The wizard exposes four explicit operations:

1. On the new device, create a provisional encrypted workspace and a
   short-lived device-signed request for an exact Account ID.
2. On an existing trusted device, inspect the request signature and freshness
   and display the derived 12-digit SAS at large size.
3. Only after the user types the independently compared SAS, ask the one-shot
   helper to enroll the exact device and create a recipient-encrypted response.
4. On the requesting device, accept that response into the exact provisional
   workspace.

Request and response fields support an explicit "drop next file" mode. A drop
is never guessed from a file extension: the user first arms the intended field.
Authorization is additionally bound to the canonical path that was inspected;
changing the path requires a new inspection.

After accept, the UI fills a secret-free launch-profile draft with the accepted
state directory, signed device list, runtime ticket, and IPC paths. Old peer
Account ID and peer prekey paths are cleared. The user must still add current
peer routing material before the ordinary launch-profile validator will save a
usable runtime profile.

## 3. Process boundary

Enrollment operations invoke the sibling `kilogram-bootstrap` process. Recovery
operations invoke the sibling `kilogram-cli` process. Both run on the existing
single-operation desktop worker so the window remains responsive and two
state-changing commands cannot be started concurrently.

The GUI passes only paths, public Account ID, conversation label, policy flags,
and an explicitly typed SAS. No seed, recovery phrase, Root key, device key,
vault key, bearer token, or message plaintext is placed on a command line.

Helper stdout and stderr are each bounded to 256 KiB. Device-link JSON denies
unknown fields, validates status, identifiers, SAS shape, absolute/workspace
paths, recipient-encryption claim, authority revision, and recovery readiness.
The GUI also verifies that returned target/output paths exactly match its
requested paths.

All enrollment and recovery state operations require the desktop-owned runtime
to be stopped/disconnected. This preserves the runtime actor as the only normal
live state writer.

## 4. Multi-source recovery view

The recipient may approve a new source-specific signed plan or add multiple
existing plan files. Per-plan rows display the last observed terminal status,
signed scheduler lifecycle, total attempts, and completion flag.

An irreversible plan cancellation is exposed only after a separate explicit
confirmation checkbox. Cancellation is written by the existing signed
scheduler transition; hiding a row in the UI is not treated as revocation.

"Run one bounded attempt" calls `history-recovery-plan-run` with one attempt and
a five-second discovery window. It is deliberately not an unbounded background
service and does not register Windows Task Scheduler. A valid policy-blocked or
scheduled run may exit nonzero; if it emitted a structured terminal status, the
UI keeps and displays that state instead of losing it as an opaque error.

Network consent is explicit per newly approved plan:

- Ethernet and Wi-Fi are allowed by default;
- mobile/metered and unknown networks are denied by default;
- external power may be required.

The reconciliation action reads all locally stored, verified source claims for
the conversation and displays exactly one of:

- `incomplete`: no complete source claim;
- `single-source`: one complete observed source;
- `agreed`: at least two complete sources claim the same inventory and no
  equivocation was observed;
- `divergent`: complete inventories disagree or a source equivocates.

Counts for observed sources, complete sources, covered events, and equivocation
are shown. `global_completeness_proven=false` is displayed explicitly even for
`agreed`: the result covers only sources the recipient actually observed.

## 5. Non-goals and limitations

- Enrollment transfers authority, not conversation membership or message
  history.
- The plan list is an in-process view of independently signed plan files; it is
  not a new unsigned source of authority.
- The source still has to publish its matching signed recovery link while the
  recipient runs an attempt.
- Wide-area source discovery, automatic source selection, persistent GUI child
  supervision, live QR camera/clipboard integration, and a global completeness
  witness are not part of this stage.
- Phrase-only Account Root restore remains blocked until current authority
  history can be authenticated without rollback or fork.

## 6. Verification

M0.9.18 includes strict JSON/path validation and structured recovery-output
tests. A release process smoke completed create, request, inspect, authorize,
accept, and an empty reconciliation. The observed authority revision was 2;
the reconciliation was `incomplete` with
`global_completeness_proven=false`.
