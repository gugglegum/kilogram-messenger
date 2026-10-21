# RFC-0279: Process-owned pending guided-recovery launch slot (M1.2.55)

Status: accepted as a dormant process-local custody boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, filesystem
discovery, or product networking.

## 1. One pending authorization, one installed host

`RecipientImportGuidedRecoveryLaunchSlot` is non-cloneable and owns at most one
complete M1.2.54 authorization and at most one installed M1.2.51 host. Its empty
constructor and authorization-install seam remain crate-private while the
feature is dormant. The slot is not serializable or recoverable after restart.

Install accepts the complete authorization by value. It rejects clock
regression and expiry before retaining custody, refuses a second pending
authorization, and refuses any authorization after a host has been installed.
Rejected values are dropped; there is no silent replacement, eviction, exact
replay exception, or API that returns the incoming authorization.

## 2. Consume before fallible launch work

The public surface exposes only a safe state enum, the existing M1.2.54 safe
preview, typed approve, and typed cancel. It never returns the issuer,
authorization, installation owner, raw host, session, hand-off, path, signer,
descriptor, coordinator, membership, or identity material.

Approve takes the pending authorization out of the slot before phrase, clock,
expiry, issuer, or host-installation checks. It delegates exact phrase checking
to M1.2.54 and stores the resulting host only after both one-shot transitions
succeed. Success returns only `HostInstalled`. Phrase mismatch, clock
regression, expiry, issuer failure, and host-installation failure all leave the
slot empty and no reusable grant. Typed cancel likewise takes and destroys the
pending authorization and returns only `Empty`; it cannot cancel or extract an
installed host.

## 3. Dormant product boundary

Ordinary startup creates no slot, installs no authorization, and approves no
launch. There is no runtime IPC command, CLI command, desktop control, timer,
background worker, filesystem-discovery path, or network registration. Peer
wire and runtime IPC wire are unchanged, and the public sync default remains
`legacy-only`.

This stage deliberately keeps the installed host opaque. A following stage may
route the already typed host controls through the same process-owned slot, but
must not expose the raw host or recover ephemeral custody after restart.

## 4. Verification

Stable-name real-storage fixtures cover the exact approval path through one
installed host, clock regression and expired install, pending and installed
replacement refusal, typed cancellation, phrase mismatch, approval expiry,
issuer validation failure, and downstream host-installation failure. Source
and manifest mutation checks protect one-slot cardinality, crate-private seams,
take-before-fallible ordering, safe return types, dormant integration, and the
unchanged product default.

The two focused slot fixtures passed 2/2 in 130.65 seconds. The combined guided
recovery regression passed 16/16 in 1648.14 seconds. Accepted-tag readiness
passed all 181/181 gates with rustfmt and workspace-wide offline strict Clippy.
