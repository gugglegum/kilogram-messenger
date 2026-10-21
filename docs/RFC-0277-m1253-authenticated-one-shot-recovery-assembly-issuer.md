# RFC-0277: Authenticated one-shot recovery-assembly issuer (M1.2.53)

Status: accepted as a dormant process-local authority boundary. It is not
registered with desktop, runtime IPC, CLI, timers, background work, filesystem
discovery, or product networking.

## 1. One complete authenticated input

`RecipientImportGuidedRecoveryAssemblyIssuer` owns the entire authenticated
input needed to assemble one guided recovery session: the exact state
directory, Account and Device, exact local signer, signed Plan request, exact
coordinator plan and sequence, and already-loaded ordered membership
snapshots. The input is private and process-local. Neither it nor the issuer
implements Clone, Debug, Serialize, or Deserialize, and neither can be
restored after a process restart.

The issuer has no public constructor. Its crate-private construction seam is
reserved for a previously authenticated canonical Plan assembly; it does not
search the filesystem or accept ambient state. Public callers cannot provide
paths, descriptors, coordinators, signers, references, or membership snapshots
as loose arguments.

## 2. Consume before validation

`issue(self)` destructures and consumes the private authenticated input before
starting fallible validation. It then creates the internal authenticated
session through a private constructor. That constructor repeats the complete
canonical verification: exact state/Account/Device, signed Plan validation,
coordinator plan ID and sequence, ordered membership references, and each
membership digest. Binding to the exact local signer is checked again before
the host-installation value can be created.

Success emits only one
`RecipientImportGuidedRecoveryHostInstallationOwner`. Failure returns no
session, host, hand-off, controller, memberships, or reusable grant. Because
the issuer is consumed on every result, neither success nor failure is
replayable.

The former loose-fragment
`AuthenticatedGuidedRecipientImportSession::from_authenticated_recovery`
constructor is now private. A separately named `#[cfg(test)]` seam exists only
to keep the older controller, local-UX, producer, and host regression fixtures
focused on their own layer; it is absent from production builds.

## 3. Dormant product boundary

Ordinary startup creates neither issuer, installation owner, nor host. The
issuer has no runtime IPC command, CLI command, desktop control, timer,
background worker, filesystem-discovery path, or network registration. Peer
wire and runtime IPC wire are unchanged, and the public sync default remains
`legacy-only`.

## 4. Verification

Stable-name real-storage fixtures cover the successful issuer → owner → host
transition and fail-closed rejection of a foreign signer, foreign state path,
and changed membership-reference set. Source and manifest mutation checks
protect constructor privacy, consume-by-value semantics, output narrowing,
full revalidation, dormant integration, and the unchanged product default.

The two focused issuer fixtures passed 2/2 in 119.36 seconds. The combined
M1.2.47–M1.2.53 guided-recovery regression passed 12/12 in 1368.98 seconds.
Accepted-tag readiness passed all 179/179 gates with rustfmt and workspace-wide
offline strict Clippy.
