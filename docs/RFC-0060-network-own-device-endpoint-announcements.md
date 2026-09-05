# RFC-0060: Network own-device endpoint announcements (M0.9.38)

Status: implemented in M0.9.38.

## 1. Problem

M0.9.37 defines the trust boundary for moving endpoint candidates and signed
publication high-water between devices of one account, but its diagnostic
ceremony requires an encrypted file to be copied manually. The payload is
already suitable for an untrusted carrier; this slice transports that exact
envelope over an authenticated online Device session without adding another
authority or import path.

## 2. Session and push preflight

The source runtime exposes IPC v12 command `PushEndpointAnnouncements`. Its
input is a fresh connection ticket published by the recipient runtime. Before
opening a connection the source requires that the ticket:

- names another Device in the source runtime's exact current Root-signed list;
- names the same Account as listener and allowed requester;
- carries a byte-identical current own-device list;
- fully verifies the recipient certificate, Root authority, prekey directory,
  route policy and ticket signature.

The source then builds the existing canonical M0.9.37 bundle in memory, signs
it as the source Device and HPKE-seals it to the exact recipient certificate.
No plaintext announcement or private publication capability is sent.

The current M0 listener ticket admits one requester Account. Consequently the
recipient runtime used for this operation must publish a ticket whose allowed
requester is its own Account. A future multi-audience listener can remove this
operational limitation without changing the announcement envelope or import
rules.

## 3. Recipient import

Iroh authenticates the endpoint named by the ticket. Before the application
frame, the normal Root- and session-bound Device authorization runs. The new
wire request contains only the recipient-encrypted M0.9.37 envelope.

The recipient additionally requires that the bundle's signed source Device is
the Device authenticated on this exact session. It then calls the same
`import_runtime_endpoint_announcement_envelope` gate used by file import. That
gate remains the only writer path and still requires exact current own roster,
existing conversation membership, immutable endpoint contracts, monotonic
publication evidence and recipient-local signatures. The network handler does
not install Root authority or membership and does not reinterpret records.

Imported public connection-ticket files live in a deterministic
`kilogram-received-endpoints` directory beside the runtime IPC descriptor (or
runtime ticket if IPC is disabled), never inside protected state. The sender
cannot choose this path. State changes remain serialized by the runtime actor
and its state/vault transaction boundary.

## 4. Replay acknowledgement

After a successful commit, the recipient signs an acknowledgement containing:

- the content-derived announcement bundle ID;
- source and recipient Device IDs;
- the current transport-session binding;
- exact Root authority revision;
- imported contact, endpoint, binding and observation counts.

The source accepts the response only from the ticket's exact recipient Device,
for the exact bundle and current session. A captured acknowledgement cannot be
used on a later connection. If delivery succeeded but the response was lost,
the same encrypted bundle can be sent again: descriptor writes are
byte-identical, append-only records are idempotent, and the recipient returns a
fresh acknowledgement bound to the retry session.

An invalid envelope receives only a generic rejection on the wire; detailed
validation errors remain local to the recipient.

## 5. Bounded backpressure

- one connection carries one announcement envelope and one acknowledgement;
- a network envelope is capped at 7 MiB, below the 8 MiB transport-frame cap;
- acknowledgement bytes are capped at 4 KiB;
- the inherited payload bound remains 256 contacts and four endpoints per
  contact;
- the runtime IPC queue and actor serialize pushes, so multiple local callers
  cannot create an unbounded network-write fan-out;
- normal connection, route and wire I/O deadlines apply.

This is online transfer, not offline mailbox delivery or high-frequency
gossip. It runs only while both clients are running and only after an explicit
IPC/desktop action.

## 6. Compatibility and command

The new wire variants are intentionally incompatible with the previous enum
encoding, so the Iroh ALPN advances from `kilogram/m0/sync/7` to
`kilogram/m0/sync/8`. Connection ticket v10, event, ratchet and announcement
envelope formats do not change. Runtime IPC advances from v11 to v12; runtime,
CLI adapter and desktop must be upgraded together.

With both same-account runtimes online and the recipient ticket synchronized:

```powershell
.\kilogram-cli.exe runtime-ipc-push-endpoint-announcements `
  --ipc-file .\source-runtime.ipc.json `
  --recipient-ticket-file .\recipient-runtime.ticket
```

The response reports the signed bundle ID, recipient-applied counts, selected
transport path and verified acknowledgement status. No `.eab` transfer file is
created.

## 7. Non-goals and next work

This slice does not discover sibling devices, create an account Device, run in
the OS background, authorize a peer Account, hide traffic correlation, provide
offline delivery or automatically retry after process restart. Seamless
foreground exchange needs a multi-audience runtime listener plus a durable,
bounded own-device endpoint/transfer schedule; optional OS autostart remains a
separate user setting.
