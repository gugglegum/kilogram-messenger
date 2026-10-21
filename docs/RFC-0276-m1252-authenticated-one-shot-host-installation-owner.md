# RFC-0276: Authenticated one-shot host-installation owner (M1.2.52)

Status: accepted as a dormant process-local ownership boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, filesystem
discovery, or product networking.

## 1. Full assembly before installation

The existing `AuthenticatedGuidedRecipientImportSession` remains the complete
canonical recovery assembly: it retains the exact state, Account, Device,
signed Plan request, coordinator plan and sequence, ordered approved-source
references, and already loaded membership snapshots. Before host ownership can
be created, that complete value is consumed and bound to the exact local
`DeviceIdentity` whose device ID it already authenticates.

The resulting host-installation value is crate-private and opaque. It has no
public constructor and no Clone, Debug, Serialize, or Deserialize
implementation. The installation owner therefore never accepts a path,
signer, descriptor, coordinator, membership reference, or membership snapshot
as separate public arguments, and it performs no ambient filesystem discovery.

## 2. One-shot owner transition

`RecipientImportGuidedRecoveryHostInstallationOwner` contains exactly one full
installation assembly. Its constructor is crate-private, and its public
`install` method consumes `self`. The assembly is destructured before host
construction or any other fallible installation work. Success returns the
M1.2.51 host; every error drops the host, authority, and owner without returning
a reusable grant. The same owner cannot be replayed, replaced, serialized, or
restored after restart.

The host constructor and authenticated install method are now private to the
owner/host module. Consequently the owner is the only code path that can create
an installed host. A second pending installation or active-session replacement
is no longer an exposed operation; the type boundary prevents it structurally.

## 3. Unchanged local UX and product boundary

Only the installed host exposes local controls: typed `prepare`, `confirm`,
`apply`, `cancel`, and `refresh` commands returning the privacy-safe M1.2.49
view. The owner returns no authority, hand-off, start command, controller,
descriptor, coordinator, or membership snapshot.

Ordinary startup creates neither owner nor host. Runtime IPC, CLI, desktop,
timers, background workers, peer wire, runtime wire, and networking remain
unchanged. The public sync default remains `legacy-only`.

## 4. Verification

Stable-name real-storage fixtures prove installation from one complete
authenticated assembly, exact signer binding, pending-host custody, explicit
cancel, and fail-closed invalid installation. The owner and assembly are
consumed before fallible work and no test opens a network path.

The two owner fixtures passed 2/2 in 117.98 seconds. The combined guided
recovery regression passed 10/10 in 1259.16 seconds. Accepted-tag readiness
passed all 178/178 gates with rustfmt and workspace-wide offline strict Clippy.

M1.2.53 strengthens the upstream side in RFC-0277: the former crate-private
loose session constructor is private, and a one-shot authenticated assembly
issuer is now the only production path that may emit this installation owner.
