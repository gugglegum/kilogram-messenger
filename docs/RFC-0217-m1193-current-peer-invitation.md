# RFC-0217: Device-signed current peer invitation

Status: accepted for M1.1.93.

## Format

A current peer invitation is a bounded JSON artifact signed by the listening
Device. It embeds the exact base64url M1.0.0-compatible connection ticket and
commits to the exact independently signed canonical sync capability carried by
that ticket. Unknown fields are rejected.

The signature binds both Account IDs, the listener Device, Account authority
revision, endpoint, publication epoch, validity window, entire ticket text,
required capability ID, and the canonical `/cbor/2` protocol. Substituting the
ticket, deleting its capability, replacing the capability, or changing the
peer binding invalidates verification.

## Compatibility

An old client does not understand the outer invitation. The exact legacy
ticket remains available only through an explicit legacy compatibility path;
it is not silently extracted and treated as proof of a current peer. A current
importer that was given this invitation must not fall back to importing a
detached legacy ticket after invitation verification fails.

The CLI can create a no-clobber artifact and inspect it without printing the
embedded ticket. Creation requires a locally owned listener ticket that
advertises the canonical candidate. Inspection independently verifies the
outer Device signature, the embedded connection ticket, its capability and
all duplicated bindings.

## Boundary

M1.1.93 does not enroll a contact, connect to a peer, write a durable floor, or
let rollout policy activate anything. It does not change the connection-ticket
wire, sync wire, runtime IPC, or public `legacy-only` default. Durable import
and monotonic pinning are M1.1.94 work.
