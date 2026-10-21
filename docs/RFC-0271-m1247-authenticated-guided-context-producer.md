# RFC-0271: Authenticated guided-context producer (M1.2.47)

Status: accepted as a process-local hand-off boundary. Ordinary product startup
still does not begin a recovery ceremony or attach the context.

## 1. The only producer

The existing guided recipient-import ceremony already performs the expensive
authority work: it loads exact live membership snapshots, checks the
Device-signed confirmation, executes the bounded import, refreshes the durable
coordinator, and persists a completed mutation receipt. M1.2.47 makes the fresh
successful `RecipientHistoryRecoveryImportMutationAdapter::dispatch` the only
producer of an `AuthenticatedGuidedRecipientImportCompletion`.

The hand-off is a Rust value with private fields, no public constructor and no
serialization implementation. A rejected or partial dispatch does not contain
one. An old completed-mutation replay deliberately preserves its M1.2.36
no-loader property and therefore also does not manufacture a new hand-off.
The caller cannot assemble the value from a coordinator, receipt, filesystem
path or membership references.

## 2. Exact completion and actor binding

Before the hand-off exists, the adapter rebuilds the coordinator through the
canonical coordinator implementation using the already authenticated approved
source set. The completed receipt must name that exact final plan and sequence.
The ordered membership references must match both their set digest and every
live membership digest.

The producer consumes that hand-off to create the short-lived M1.2.46 context.
The runtime actor rechecks its own state directory, Account and Device before
installing it. The raw context constructor is crate-private. This closes the
remaining same-Account cross-profile gap in the M1.2.46 slot and prevents an
external integration caller from presenting independently assembled recovery
fragments.

## 3. Loopback consumption proof

A real durable recovery fixture performs the existing Plan, confirmation and
mutation ceremony and passes its fresh hand-off directly to the process-owned
actor. Canonical encoded Plan bytes produce a verified signed response; exact
Plan replay is byte-identical. Explicit Commit consumes the intent once, and a
lost-response replay returns durable `AlreadyConsumed` without a second
consume. Explicit detach clears the opaque Plan stage and all later requests
fail as unattached.

The negative proof rejects foreign state, Account and Device bindings. Existing
mutation regressions also prove changed membership/nonce and rejected dispatch
do not gain a hand-off, while exact completed replay remains loader-free and
therefore emits none.

## 4. Product boundary

This milestone connects two already implemented in-process components; it does
not add a user-visible way to start them. Ordinary runtime startup remains
unattached. No hand-off or live membership snapshot crosses IPC, CLI,
environment, filesystem scan or desktop control. No timer or product network
path is added, peer and runtime IPC wire stay unchanged, and the public sync
default remains `legacy-only`.

The next stage may add one runtime-owned guided recovery session controller,
but it must keep startup unattached and require an explicit local ceremony. It
must not turn the new producer into ambient background authority.

M1.2.48 implemented that successor as the dormant controller specified by
RFC-0272. It preserves every boundary above: the completion hand-off remains
process-local, ordinary startup remains idle/unattached, and no CLI, desktop,
runtime-IPC or network entry point was added.
