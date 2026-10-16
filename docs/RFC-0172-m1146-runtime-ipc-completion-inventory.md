# RFC-0172: M1.1.46 exhaustive runtime IPC completion inventory

Status: implemented as an inactive completion audit in M1.1.46.

## Purpose

The canonical runtime IPC work had accumulated many independently bounded
command families, while the aggregate coverage record still used one vague
catch-all for everything not yet enumerated. That made it possible to miss an
active local-control command even though every existing canonical module was
individually green.

M1.1.46 replaces that catch-all with a machine-checked inventory. The inventory
is exhaustive for both sides of the comparison:

- all 32 canonical commands and all 31 canonical results;
- all 34 commands and all 31 responses in the active Postcard v27 adapter;
- the exact canonical command/result pairing and owning module; and
- the disposition of every active adapter variant.

This audit does not make the canonical target complete. It makes incompleteness
explicit and fail closed.

## Remaining canonical work

Three concrete decisions or protocol additions remain:

1. ticket automation policy and status need a canonical contract or an explicit
   removal decision;
2. endpoint-announcement bootstrap helpers need a final disposition against the
   already canonical announcement and own-device automation surfaces; and
3. legacy central-HTTPS mailbox commands must be removed before canonical local
   control can become active, because the service-free exact volunteer commands
   already replace them.

The active adapter therefore has seven Postcard-only activation blockers:
`CreateMailboxCapability`, `RotateMailboxCapability`,
`ConfigureTicketAutomation`, `TicketAutomationStatus`,
`ExportEndpointAnnouncements`, `ImportEndpointAnnouncements`, and
`PushEndpointAnnouncements`.

## Fail-closed boundary

`CANONICAL-RUNTIME-IPC-COMPLETION.json` is the normative completion inventory.
Its dedicated boundary rejects:

- a canonical command without one exact result mapping;
- a canonical result that is not reachable from a mapped command or the
  universal error result;
- any active Rust enum variant absent from the inventory;
- an unknown adapter disposition or canonical target;
- omission of any one of the seven blockers;
- a changed set of the three remaining work items; and
- any premature claim that the canonical target or active adapter is complete.

The boundary derives the active variant list directly from the Rust enums so a
future Postcard command cannot silently bypass the completion ledger.

## Activation boundary

M1.1.46 does not activate canonical CBOR and does not change any active wire,
including the local Postcard control channel and every peer network protocol.
Activation still requires an explicit protocol identity, a parallel reader and
writer with downgrade tests, removal or canonicalization of every blocker,
atomic durable adapters, sustained fuzzing, independent HPKE conformance, and
fresh field evidence from desktop and CLI multi-device operation.

