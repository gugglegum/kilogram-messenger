# RFC-0280: Opaque installed-host command router (M1.2.56)

Status: accepted as a dormant process-local command boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, filesystem
discovery, or product networking.

## 1. Route commands, never the host

The M1.2.55 process-owned launch slot now routes the five existing typed host
commands: prepare, confirm, apply, cancel, and refresh. Every route borrows the
installed host only inside the slot and returns the existing M1.2.49
`RecipientImportGuidedSessionLocalUxView`. No method returns or lends the raw
host, installation owner, authorization, issuer, authenticated session, start
hand-off, path, signer, descriptor, coordinator, membership, Account ID, or
Device ID.

The router adds no second lifecycle. It delegates to the existing M1.2.51 host,
which in turn remains the sole composition of the controller, sealed one-shot
start hand-off, producer, and privacy-safe local UX adapter.

## 2. Installed-host state is mandatory

All five commands pass through one private `installed_host_mut` gate. The gate
rejects both `Empty` and `PendingAuthorization` before host, controller, or
durable work. It also rejects an internally inconsistent state that still owns
a pending authorization, even if a host were present.

The pending launch preview and exact approval ceremony remain unchanged. A
pending authorization cannot be used as an implicit host and routed commands
cannot approve, consume, cancel, or otherwise disturb it.

## 3. Failure keeps custody opaque

Host and local-UX errors stay typed under the existing slot error. A routed
error does not remove, return, replace, or expose the installed host. The host
continues to own its controller and whatever fail-closed state that controller
retains. A new authorization is still rejected after a routed error because
the slot continues to own one host.

The slot and host remain non-cloneable, non-serializable, and unrecoverable
after process restart. There is no accessor, `take`, `into`, replacement, or
restart seam for installed custody.

## 4. Dormant product boundary

Ordinary startup still creates no slot and routes no command. There is no new
runtime IPC request, CLI subcommand, desktop control, timer, background worker,
filesystem discovery, or network registration. Peer wire and runtime IPC wire
are unchanged, and the public sync default remains `legacy-only`.

## 5. Verification

Stable-name real-storage fixtures cover the complete
prepare-confirm-apply-refresh-cancel lifecycle through the slot. A second
fixture proves that every command rejects both empty and pending states,
pending authorization remains intact, a routed host error leaves the host
installed and usable, and replacement remains forbidden.

The two focused router fixtures passed 2/2 in 271.86 seconds. The combined
guided-recovery regression passed 18/18 in 1926.66 seconds. Accepted-tag
readiness passed all 182/182 gates with rustfmt and workspace-wide locked,
offline strict Clippy. Source and manifest mutation checks rejected 16/16
weakenings and protect all five routes, the common installed-host gate,
privacy-safe return type, retained custody after errors, dormant integration,
and unchanged product default.
